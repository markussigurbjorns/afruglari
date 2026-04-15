use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VarId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(u8),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Domain {
    Bool { can_false: bool, can_true: bool },
    SmallInt { bits: u64 },
}

impl Domain {
    pub fn bool() -> Self {
        Self::Bool {
            can_false: true,
            can_true: true,
        }
    }

    pub fn bool_fixed(value: bool) -> Self {
        Self::Bool {
            can_false: !value,
            can_true: value,
        }
    }

    pub fn small_range(min: u8, max: u8) -> Self {
        assert!(min <= max, "empty integer range");
        assert!(max < 64, "SmallInt supports values 0..63");

        let mut bits = 0;
        for value in min..=max {
            bits |= 1_u64 << value;
        }

        Self::SmallInt { bits }
    }

    pub fn is_empty(&self) -> bool {
        match *self {
            Self::Bool {
                can_false,
                can_true,
            } => !can_false && !can_true,
            Self::SmallInt { bits } => bits == 0,
        }
    }

    pub fn is_singleton(&self) -> bool {
        self.size() == 1
    }

    pub fn size(&self) -> usize {
        match *self {
            Self::Bool {
                can_false,
                can_true,
            } => usize::from(can_false) + usize::from(can_true),
            Self::SmallInt { bits } => bits.count_ones() as usize,
        }
    }

    pub fn contains(&self, value: Value) -> bool {
        match (self, value) {
            (
                Self::Bool {
                    can_false,
                    can_true,
                },
                Value::Bool(value),
            ) => {
                if value {
                    *can_true
                } else {
                    *can_false
                }
            }
            (Self::SmallInt { bits }, Value::Int(value)) => {
                value < 64 && (bits & (1_u64 << value)) != 0
            }
            _ => false,
        }
    }

    pub fn singleton_value(&self) -> Option<Value> {
        match *self {
            Self::Bool {
                can_false: true,
                can_true: false,
            } => Some(Value::Bool(false)),
            Self::Bool {
                can_false: false,
                can_true: true,
            } => Some(Value::Bool(true)),
            Self::SmallInt { bits } if bits.count_ones() == 1 => {
                Some(Value::Int(bits.trailing_zeros() as u8))
            }
            _ => None,
        }
    }

    pub fn values(&self) -> Vec<Value> {
        match *self {
            Self::Bool {
                can_false,
                can_true,
            } => {
                let mut values = Vec::with_capacity(2);
                if can_false {
                    values.push(Value::Bool(false));
                }
                if can_true {
                    values.push(Value::Bool(true));
                }
                values
            }
            Self::SmallInt { bits } => {
                let mut values = Vec::with_capacity(bits.count_ones() as usize);
                for value in 0..64 {
                    if (bits & (1_u64 << value)) != 0 {
                        values.push(Value::Int(value as u8));
                    }
                }
                values
            }
        }
    }

    pub(crate) fn assigned(value: Value) -> Self {
        match value {
            Value::Bool(value) => Self::bool_fixed(value),
            Value::Int(value) => Self::SmallInt {
                bits: 1_u64 << value,
            },
        }
    }

    pub(crate) fn without(&self, value: Value) -> Self {
        match (self, value) {
            (
                Self::Bool {
                    can_false,
                    can_true,
                },
                Value::Bool(value),
            ) => {
                if value {
                    Self::Bool {
                        can_false: *can_false,
                        can_true: false,
                    }
                } else {
                    Self::Bool {
                        can_false: false,
                        can_true: *can_true,
                    }
                }
            }
            (Self::SmallInt { bits }, Value::Int(value)) if value < 64 => Self::SmallInt {
                bits: bits & !(1_u64 << value),
            },
            _ => self.clone(),
        }
    }

    pub(crate) fn intersect(&self, other: &Self) -> Option<Self> {
        let intersection = match (self, other) {
            (
                Self::Bool {
                    can_false: left_false,
                    can_true: left_true,
                },
                Self::Bool {
                    can_false: right_false,
                    can_true: right_true,
                },
            ) => Self::Bool {
                can_false: *left_false && *right_false,
                can_true: *left_true && *right_true,
            },
            (Self::SmallInt { bits: left }, Self::SmallInt { bits: right }) => {
                Self::SmallInt { bits: left & right }
            }
            _ => return None,
        };

        if intersection.is_empty() {
            None
        } else {
            Some(intersection)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrailEntry {
    var: VarId,
    old_domain: Domain,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Level {
    trail_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Conflict {
    pub message: String,
}

impl Conflict {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Conflict {}

pub trait Constraint {
    fn scope(&self) -> &[VarId];

    fn propagate(&self, engine: &mut Engine, changed: VarId) -> Result<(), Conflict>;

    fn is_satisfied_complete(&self, engine: &Engine) -> bool;
}

pub struct Engine {
    domains: Vec<Domain>,
    constraints: Vec<Rc<dyn Constraint>>,
    watchers: Vec<Vec<usize>>,
    trail: Vec<TrailEntry>,
    levels: Vec<Level>,
    queue: VecDeque<VarId>,
}

impl Engine {
    pub fn new(domains: Vec<Domain>) -> Self {
        let watchers = vec![Vec::new(); domains.len()];
        Self {
            domains,
            constraints: Vec::new(),
            watchers,
            trail: Vec::new(),
            levels: Vec::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn add_constraint(&mut self, constraint: impl Constraint + 'static) {
        let constraint_index = self.constraints.len();
        for var in constraint.scope() {
            self.watchers[var.0].push(constraint_index);
        }
        self.constraints.push(Rc::new(constraint));
    }

    pub fn domain(&self, var: VarId) -> &Domain {
        &self.domains[var.0]
    }

    pub fn value(&self, var: VarId) -> Option<Value> {
        self.domain(var).singleton_value()
    }

    pub fn assign(&mut self, var: VarId, value: Value) -> Result<(), Conflict> {
        self.restrict(var, Domain::assigned(value))
    }

    pub fn remove_value(&mut self, var: VarId, value: Value) -> Result<(), Conflict> {
        self.restrict(var, self.domain(var).without(value))
    }

    pub fn restrict(&mut self, var: VarId, new_domain: Domain) -> Result<(), Conflict> {
        let Some(restricted) = self.domain(var).intersect(&new_domain) else {
            return Err(Conflict::new(format!("domain wipeout for {:?}", var)));
        };

        if restricted == *self.domain(var) {
            return Ok(());
        }

        let old_domain = std::mem::replace(&mut self.domains[var.0], restricted);
        self.trail.push(TrailEntry { var, old_domain });
        self.queue.push_back(var);
        Ok(())
    }

    pub fn push_level(&mut self) {
        self.levels.push(Level {
            trail_len: self.trail.len(),
        });
    }

    pub fn pop_level(&mut self) {
        let level = self.levels.pop().expect("pop_level without push_level");
        while self.trail.len() > level.trail_len {
            let entry = self.trail.pop().expect("trail length checked");
            self.domains[entry.var.0] = entry.old_domain;
        }
        self.queue.clear();
    }

    pub fn propagate_all(&mut self) -> Result<(), Conflict> {
        if self.queue.is_empty() {
            for var_index in 0..self.domains.len() {
                self.queue.push_back(VarId(var_index));
            }
        }

        while let Some(changed) = self.queue.pop_front() {
            let watchers = self.watchers[changed.0].clone();
            for constraint_index in watchers {
                let constraint = Rc::clone(&self.constraints[constraint_index]);
                constraint.propagate(self, changed)?;
            }
        }

        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        self.domains.iter().all(Domain::is_singleton)
    }

    pub fn is_satisfied_complete(&self) -> bool {
        self.constraints
            .iter()
            .all(|constraint| constraint.is_satisfied_complete(self))
    }

    pub fn len(&self) -> usize {
        self.domains.len()
    }

    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }
}

pub fn solve(engine: &mut Engine) -> bool {
    solve_with_seed(engine, 0)
}

pub fn solve_with_seed(engine: &mut Engine, seed: u64) -> bool {
    if engine.propagate_all().is_err() {
        return false;
    }
    if engine.is_complete() {
        return engine.is_satisfied_complete();
    }

    let var = choose_var(engine, seed).expect("incomplete engine has a variable to choose");

    for value in ordered_values(var, engine.domain(var), seed) {
        engine.push_level();
        if engine.assign(var, value).is_ok() && solve_with_seed(engine, next_seed(seed, var, value))
        {
            return true;
        }
        engine.pop_level();
    }

    false
}

fn choose_var(engine: &Engine, seed: u64) -> Option<VarId> {
    engine
        .domains
        .iter()
        .enumerate()
        .filter(|(_, domain)| domain.size() > 1)
        .min_by_key(|(index, domain)| (domain.size(), mix(seed ^ *index as u64)))
        .map(|(index, _)| VarId(index))
}

fn ordered_values(var: VarId, domain: &Domain, seed: u64) -> Vec<Value> {
    let mut values = domain.values();
    let max_int = values
        .iter()
        .filter_map(|value| match value {
            Value::Int(value) => Some(*value),
            Value::Bool(_) => None,
        })
        .max();
    let high_first = mix(seed ^ var.0 as u64) & 1 == 0;
    values.sort_by_key(|value| match *value {
        Value::Bool(true) => mix(seed ^ var.0 as u64 ^ 0x7a),
        Value::Bool(false) => mix(seed ^ var.0 as u64 ^ 0xf0),
        Value::Int(value) if high_first && Some(value) == max_int => 0,
        Value::Int(0) if !high_first => 0,
        Value::Int(0) => 1,
        Value::Int(value) if Some(value) == max_int => 1,
        Value::Int(value) => 10 + (mix(seed ^ var.0 as u64 ^ value as u64) % 64),
    });
    values
}

fn next_seed(seed: u64, var: VarId, value: Value) -> u64 {
    let value_bits = match value {
        Value::Bool(false) => 0x2f,
        Value::Bool(true) => 0x83,
        Value::Int(value) => value as u64,
    };
    mix(seed ^ ((var.0 as u64) << 8) ^ value_bits)
}

fn mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
