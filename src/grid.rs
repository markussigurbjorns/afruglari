use crate::csp::{Domain, Engine, Value, VarId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Param {
    Active,
    Register,
    Timbre,
    Intensity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventDurationMode {
    SingleStep,
    MergeAdjacent,
}

#[derive(Clone, Debug)]
pub struct Grid {
    voices: usize,
    steps: usize,
    params_per_cell: usize,
}

impl Grid {
    pub fn new(voices: usize, steps: usize) -> Self {
        Self {
            voices,
            steps,
            params_per_cell: 4,
        }
    }

    pub fn domains(&self, registers: u8, timbres: u8, intensities: u8) -> Vec<Domain> {
        let mut domains = Vec::with_capacity(self.voices * self.steps * self.params_per_cell);
        for _voice in 0..self.voices {
            for _step in 0..self.steps {
                domains.push(Domain::bool());
                domains.push(Domain::small_range(0, registers));
                domains.push(Domain::small_range(0, timbres));
                domains.push(Domain::small_range(0, intensities));
            }
        }
        domains
    }

    pub fn var(&self, voice: usize, step: usize, param: Param) -> VarId {
        assert!(voice < self.voices, "voice out of range");
        assert!(step < self.steps, "step out of range");
        let param_offset = match param {
            Param::Active => 0,
            Param::Register => 1,
            Param::Timbre => 2,
            Param::Intensity => 3,
        };
        VarId((voice * self.steps + step) * self.params_per_cell + param_offset)
    }

    pub fn voice_param(&self, voice: usize, param: Param) -> Vec<VarId> {
        (0..self.steps)
            .map(|step| self.var(voice, step, param))
            .collect()
    }

    pub fn all_active(&self) -> Vec<VarId> {
        let mut vars = Vec::with_capacity(self.voices * self.steps);
        for step in 0..self.steps {
            for voice in 0..self.voices {
                vars.push(self.var(voice, step, Param::Active));
            }
        }
        vars
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub voice: usize,
    pub step: usize,
    pub duration_steps: usize,
    pub register: Option<u8>,
    pub timbre: u8,
    pub intensity: u8,
}

pub fn events_from_grid(engine: &Engine, grid: &Grid) -> Vec<Event> {
    events_from_grid_with_durations(engine, grid, EventDurationMode::SingleStep, 1)
}

pub fn events_from_grid_with_durations(
    engine: &Engine,
    grid: &Grid,
    mode: EventDurationMode,
    max_duration_steps: usize,
) -> Vec<Event> {
    let mut events = Vec::new();
    let max_duration_steps = max_duration_steps.max(1);

    for voice in 0..grid.voices {
        let mut step = 0;
        while step < grid.steps {
            if engine.value(grid.var(voice, step, Param::Active)) != Some(Value::Bool(true)) {
                step += 1;
                continue;
            }

            let register = match engine.value(grid.var(voice, step, Param::Register)) {
                Some(Value::Int(value)) => Some(value),
                _ => None,
            };
            let timbre = match engine.value(grid.var(voice, step, Param::Timbre)) {
                Some(Value::Int(value)) => value,
                _ => 0,
            };
            let intensity = match engine.value(grid.var(voice, step, Param::Intensity)) {
                Some(Value::Int(value)) => value,
                _ => 0,
            };

            let mut duration_steps = 1;
            if mode == EventDurationMode::MergeAdjacent {
                while step + duration_steps < grid.steps && duration_steps < max_duration_steps {
                    let next_step = step + duration_steps;
                    if engine.value(grid.var(voice, next_step, Param::Active))
                        != Some(Value::Bool(true))
                    {
                        break;
                    }

                    let next_register =
                        match engine.value(grid.var(voice, next_step, Param::Register)) {
                            Some(Value::Int(value)) => Some(value),
                            _ => None,
                        };
                    let next_timbre = match engine.value(grid.var(voice, next_step, Param::Timbre))
                    {
                        Some(Value::Int(value)) => value,
                        _ => 0,
                    };
                    let next_intensity =
                        match engine.value(grid.var(voice, next_step, Param::Intensity)) {
                            Some(Value::Int(value)) => value,
                            _ => 0,
                        };

                    if next_register != register
                        || next_timbre != timbre
                        || next_intensity != intensity
                    {
                        break;
                    }

                    duration_steps += 1;
                }
            }

            events.push(Event {
                voice,
                step,
                duration_steps,
                register,
                timbre,
                intensity,
            });
            step += duration_steps;
        }
    }

    events
}
