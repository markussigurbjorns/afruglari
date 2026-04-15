use crate::csp::{Conflict, Constraint, Engine, Value, VarId};

#[derive(Clone, Debug)]
pub struct MaxRun {
    scope: Vec<VarId>,
    max_true: usize,
}

impl MaxRun {
    pub fn new(scope: Vec<VarId>, max_true: usize) -> Self {
        Self { scope, max_true }
    }
}

impl Constraint for MaxRun {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        if self.scope.len() <= self.max_true {
            return Ok(());
        }

        for segment in self.scope.windows(self.max_true + 1) {
            let true_count = segment
                .iter()
                .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
                .count();

            if true_count > self.max_true {
                return Err(Conflict::new("max run exceeded"));
            }

            if true_count == self.max_true {
                for var in segment {
                    if engine.domain(*var).contains(Value::Bool(false)) {
                        engine.remove_value(*var, Value::Bool(true))?;
                    }
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.scope.windows(self.max_true + 1).all(|segment| {
            segment
                .iter()
                .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
                .count()
                <= self.max_true
        })
    }
}

#[derive(Clone, Debug)]
pub struct ExactCount {
    scope: Vec<VarId>,
    value: Value,
    count: usize,
}

impl ExactCount {
    pub fn new(scope: Vec<VarId>, value: Value, count: usize) -> Self {
        Self {
            scope,
            value,
            count,
        }
    }
}

impl Constraint for ExactCount {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        let assigned = self
            .scope
            .iter()
            .filter(|var| engine.value(**var) == Some(self.value))
            .count();
        let possible = self
            .scope
            .iter()
            .filter(|var| engine.domain(**var).contains(self.value))
            .count();

        if assigned > self.count || possible < self.count {
            return Err(Conflict::new("exact count cannot be satisfied"));
        }

        if assigned == self.count {
            for var in &self.scope {
                if engine.value(*var) != Some(self.value) {
                    engine.remove_value(*var, self.value)?;
                }
            }
        } else if possible == self.count {
            for var in &self.scope {
                if engine.domain(*var).contains(self.value) {
                    engine.assign(*var, self.value)?;
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.scope
            .iter()
            .filter(|var| engine.value(**var) == Some(self.value))
            .count()
            == self.count
    }
}

#[derive(Clone, Debug)]
pub struct MinCount {
    scope: Vec<VarId>,
    value: Value,
    count: usize,
}

impl MinCount {
    pub fn new(scope: Vec<VarId>, value: Value, count: usize) -> Self {
        Self {
            scope,
            value,
            count,
        }
    }
}

impl Constraint for MinCount {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        let assigned = self
            .scope
            .iter()
            .filter(|var| engine.value(**var) == Some(self.value))
            .count();
        let possible = self
            .scope
            .iter()
            .filter(|var| engine.domain(**var).contains(self.value))
            .count();

        if possible < self.count {
            return Err(Conflict::new("minimum count cannot be satisfied"));
        }

        if assigned < self.count && possible == self.count {
            for var in &self.scope {
                if engine.domain(*var).contains(self.value) {
                    engine.assign(*var, self.value)?;
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.scope
            .iter()
            .filter(|var| engine.value(**var) == Some(self.value))
            .count()
            >= self.count
    }
}

#[derive(Clone, Debug)]
pub struct MaxCount {
    scope: Vec<VarId>,
    value: Value,
    count: usize,
}

impl MaxCount {
    pub fn new(scope: Vec<VarId>, value: Value, count: usize) -> Self {
        Self {
            scope,
            value,
            count,
        }
    }
}

impl Constraint for MaxCount {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        let assigned = self
            .scope
            .iter()
            .filter(|var| engine.value(**var) == Some(self.value))
            .count();

        if assigned > self.count {
            return Err(Conflict::new("maximum count exceeded"));
        }

        if assigned == self.count {
            for var in &self.scope {
                if engine.value(*var) != Some(self.value) {
                    engine.remove_value(*var, self.value)?;
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.scope
            .iter()
            .filter(|var| engine.value(**var) == Some(self.value))
            .count()
            <= self.count
    }
}

#[derive(Clone, Debug)]
pub struct MinDensityWindow {
    scope: Vec<VarId>,
    window: usize,
    min_active: usize,
}

impl MinDensityWindow {
    pub fn new(scope: Vec<VarId>, window: usize, min_active: usize) -> Self {
        assert!(window > 0, "density window must be non-zero");
        Self {
            scope,
            window,
            min_active,
        }
    }
}

impl Constraint for MinDensityWindow {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        for segment in self.scope.windows(self.window) {
            let assigned = segment
                .iter()
                .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
                .count();
            let possible = segment
                .iter()
                .filter(|var| engine.domain(**var).contains(Value::Bool(true)))
                .count();

            if possible < self.min_active {
                return Err(Conflict::new("minimum density cannot be satisfied"));
            }

            if assigned < self.min_active && possible == self.min_active {
                for var in segment {
                    if engine.domain(*var).contains(Value::Bool(true)) {
                        engine.assign(*var, Value::Bool(true))?;
                    }
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.scope.windows(self.window).all(|segment| {
            segment
                .iter()
                .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
                .count()
                >= self.min_active
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Literal {
    pub var: VarId,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct Implication {
    scope: [VarId; 2],
    premise: Literal,
    conclusion: Literal,
}

impl Implication {
    pub fn new(premise: Literal, conclusion: Literal) -> Self {
        Self {
            scope: [premise.var, conclusion.var],
            premise,
            conclusion,
        }
    }
}

impl Constraint for Implication {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        if engine.value(self.premise.var) == Some(self.premise.value) {
            engine.assign(self.conclusion.var, self.conclusion.value)?;
        }

        if !engine
            .domain(self.conclusion.var)
            .contains(self.conclusion.value)
        {
            engine.remove_value(self.premise.var, self.premise.value)?;
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        engine.value(self.premise.var) != Some(self.premise.value)
            || engine.value(self.conclusion.var) == Some(self.conclusion.value)
    }
}

#[derive(Clone, Debug)]
pub struct SlowChange {
    scope: Vec<VarId>,
    window: usize,
}

impl SlowChange {
    pub fn new(scope: Vec<VarId>, window: usize) -> Self {
        assert!(window > 0, "slow-change window must be non-zero");
        Self { scope, window }
    }
}

impl Constraint for SlowChange {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        for block_start in (0..self.scope.len()).step_by(self.window) {
            let block_end = (block_start + self.window).min(self.scope.len());
            let block = &self.scope[block_start..block_end];
            let mut common = engine.domain(block[0]).clone();
            for var in &block[1..] {
                common = common
                    .intersect(engine.domain(*var))
                    .ok_or_else(|| Conflict::new("slow-change block has no common value"))?;
            }

            for var in block {
                engine.restrict(*var, common.clone())?;
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        (0..self.scope.len())
            .step_by(self.window)
            .all(|block_start| {
                let block_end = (block_start + self.window).min(self.scope.len());
                let expected = engine.value(self.scope[block_start]);
                self.scope[block_start..block_end]
                    .iter()
                    .all(|var| engine.value(*var) == expected)
            })
    }
}

#[derive(Clone, Debug)]
pub struct DifferentAdjacent {
    scope: Vec<VarId>,
}

impl DifferentAdjacent {
    pub fn new(scope: Vec<VarId>) -> Self {
        Self { scope }
    }
}

impl Constraint for DifferentAdjacent {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        for pair in self.scope.windows(2) {
            let left = pair[0];
            let right = pair[1];
            if let Some(value) = engine.value(left) {
                engine.remove_value(right, value)?;
            }
            if let Some(value) = engine.value(right) {
                engine.remove_value(left, value)?;
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.scope
            .windows(2)
            .all(|pair| engine.value(pair[0]) != engine.value(pair[1]))
    }
}

#[derive(Clone, Debug)]
pub struct AntiRepeatWindow {
    scope: Vec<VarId>,
    window: usize,
    max_repeats: usize,
}

impl AntiRepeatWindow {
    pub fn new(scope: Vec<VarId>, window: usize, max_repeats: usize) -> Self {
        assert!(window > 0, "anti-repeat window must be non-zero");
        Self {
            scope,
            window,
            max_repeats,
        }
    }
}

impl Constraint for AntiRepeatWindow {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        for segment in self.scope.windows(self.window) {
            let values = values_in_segment(engine, segment);
            for value in values {
                let assigned = segment
                    .iter()
                    .filter(|var| engine.value(**var) == Some(value))
                    .count();

                if assigned > self.max_repeats {
                    return Err(Conflict::new("anti-repeat window exceeded"));
                }

                if assigned == self.max_repeats {
                    for var in segment {
                        if engine.value(*var) != Some(value) {
                            engine.remove_value(*var, value)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.scope.windows(self.window).all(|segment| {
            values_in_segment(engine, segment).into_iter().all(|value| {
                segment
                    .iter()
                    .filter(|var| engine.value(**var) == Some(value))
                    .count()
                    <= self.max_repeats
            })
        })
    }
}

fn values_in_segment(engine: &Engine, segment: &[VarId]) -> Vec<Value> {
    let mut values = Vec::new();
    for var in segment {
        for value in engine.domain(*var).values() {
            if !values.contains(&value) {
                values.push(value);
            }
        }
    }
    values
}

#[derive(Clone, Debug)]
pub struct AtLeastCollisions {
    scope: Vec<VarId>,
    pairs: Vec<(VarId, VarId)>,
    min_count: usize,
}

#[derive(Clone, Debug)]
pub struct PhaseResponse {
    scope: Vec<VarId>,
    pairs: Vec<(VarId, VarId)>,
    min_count: usize,
}

impl PhaseResponse {
    pub fn new(pairs: Vec<(VarId, VarId)>, min_count: usize) -> Self {
        let mut scope = Vec::with_capacity(pairs.len() * 2);
        for (left, right) in &pairs {
            scope.push(*left);
            scope.push(*right);
        }
        Self {
            scope,
            pairs,
            min_count,
        }
    }
}

impl Constraint for PhaseResponse {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        let satisfied = self
            .pairs
            .iter()
            .filter(|(source, response)| {
                engine.value(*source) == Some(Value::Bool(true))
                    && engine.value(*response) == Some(Value::Bool(true))
            })
            .count();
        let possible = self
            .pairs
            .iter()
            .filter(|(source, response)| {
                engine.domain(*source).contains(Value::Bool(true))
                    && engine.domain(*response).contains(Value::Bool(true))
            })
            .count();

        if possible < self.min_count {
            return Err(Conflict::new("phase response cannot be satisfied"));
        }

        if satisfied < self.min_count && possible == self.min_count {
            for (source, response) in &self.pairs {
                if engine.domain(*source).contains(Value::Bool(true))
                    && engine.domain(*response).contains(Value::Bool(true))
                {
                    engine.assign(*source, Value::Bool(true))?;
                    engine.assign(*response, Value::Bool(true))?;
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.pairs
            .iter()
            .filter(|(source, response)| {
                engine.value(*source) == Some(Value::Bool(true))
                    && engine.value(*response) == Some(Value::Bool(true))
            })
            .count()
            >= self.min_count
    }
}

impl AtLeastCollisions {
    pub fn new(pairs: Vec<(VarId, VarId)>, min_count: usize) -> Self {
        let mut scope = Vec::with_capacity(pairs.len() * 2);
        for (left, right) in &pairs {
            scope.push(*left);
            scope.push(*right);
        }
        Self {
            scope,
            pairs,
            min_count,
        }
    }
}

impl Constraint for AtLeastCollisions {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        let satisfied = self
            .pairs
            .iter()
            .filter(|(left, right)| {
                engine.value(*left) == Some(Value::Bool(true))
                    && engine.value(*right) == Some(Value::Bool(true))
            })
            .count();
        let possible = self
            .pairs
            .iter()
            .filter(|(left, right)| {
                engine.domain(*left).contains(Value::Bool(true))
                    && engine.domain(*right).contains(Value::Bool(true))
            })
            .count();

        if possible < self.min_count {
            return Err(Conflict::new("not enough collisions possible"));
        }

        if satisfied < self.min_count && possible == self.min_count {
            for (left, right) in &self.pairs {
                if engine.domain(*left).contains(Value::Bool(true))
                    && engine.domain(*right).contains(Value::Bool(true))
                {
                    engine.assign(*left, Value::Bool(true))?;
                    engine.assign(*right, Value::Bool(true))?;
                }
            }
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        self.pairs
            .iter()
            .filter(|(left, right)| {
                engine.value(*left) == Some(Value::Bool(true))
                    && engine.value(*right) == Some(Value::Bool(true))
            })
            .count()
            >= self.min_count
    }
}

#[derive(Clone, Debug)]
pub struct MoreTrueThan {
    scope: Vec<VarId>,
    left: Vec<VarId>,
    right: Vec<VarId>,
}

impl MoreTrueThan {
    pub fn new(left: Vec<VarId>, right: Vec<VarId>) -> Self {
        let mut scope = left.clone();
        scope.extend(right.iter().copied());
        Self { scope, left, right }
    }
}

impl Constraint for MoreTrueThan {
    fn scope(&self) -> &[VarId] {
        &self.scope
    }

    fn propagate(&self, engine: &mut Engine, _changed: VarId) -> Result<(), Conflict> {
        let left_assigned = self
            .left
            .iter()
            .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
            .count();
        let left_possible = self
            .left
            .iter()
            .filter(|var| engine.domain(**var).contains(Value::Bool(true)))
            .count();
        let right_assigned = self
            .right
            .iter()
            .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
            .count();
        let right_possible = self
            .right
            .iter()
            .filter(|var| engine.domain(**var).contains(Value::Bool(true)))
            .count();

        if left_possible <= right_assigned {
            return Err(Conflict::new("left side cannot be denser than right side"));
        }

        if left_possible == right_assigned + 1 {
            for var in &self.left {
                if engine.domain(*var).contains(Value::Bool(true)) {
                    engine.assign(*var, Value::Bool(true))?;
                }
            }
            for var in &self.right {
                if engine.value(*var) != Some(Value::Bool(true)) {
                    engine.remove_value(*var, Value::Bool(true))?;
                }
            }
        }

        if left_assigned > right_possible {
            return Ok(());
        }

        Ok(())
    }

    fn is_satisfied_complete(&self, engine: &Engine) -> bool {
        let left_count = self
            .left
            .iter()
            .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
            .count();
        let right_count = self
            .right
            .iter()
            .filter(|var| engine.value(**var) == Some(Value::Bool(true)))
            .count();
        left_count > right_count
    }
}
