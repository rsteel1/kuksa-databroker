// c15_e08_build_calibration_pipeline.rs
//
// CONCEPT: function pointers (`fn`) for a fixed-size, const-generic
//          pipeline versus `Box<dyn Fn>` for a dynamic one that needs to
//          capture state — the two are not interchangeable, and this
//          exercise is about knowing which one a given job calls for.
// GUIDE:   chapters/15-advanced-types-and-layout/guide.md  (section "Function pointers, closures and Box<dyn Fn>")
// DATABROKER: providers apply calibration/scaling to raw sensor values
//          before they reach the broker; a fixed pipeline of stateless
//          steps is a `[fn(f64) -> f64; N]`, one with a runtime-configured
//          offset needs `Box<dyn Fn(f64) -> f64>` to capture that offset.
// EXPECT:  compiles; every test fails (`todo!()` panics) until you fill in
//          the four `todo!()` bodies below.
//
// YOUR TASK
//   Build this from the spec below. Nothing here is a bug to fix — you are
//   writing new code. Do not edit the tests, the structs, or the signatures.
//
//   1. `Pipeline::new`: just stores the stages.
//   2. `Pipeline::run`: feeds `input` through every stage in order, left to
//      right (`stages[0]` first), returning the final value.
//   3. `DynPipeline::add_stage`: boxes `stage` and appends it. `stage` may
//      capture its environment (that's why it can't be a plain `fn`).
//   4. `DynPipeline::run`: same left-to-right feeding as `Pipeline::run`.

/// A fixed-size pipeline of stateless transforms, known at compile time.
struct Pipeline<const N: usize> {
    stages: [fn(f64) -> f64; N],
}

impl<const N: usize> Pipeline<N> {
    fn new(stages: [fn(f64) -> f64; N]) -> Self {
        Pipeline { stages }
    }

    /// Runs `input` through every stage, left to right.
    fn run(&self, input: f64) -> f64 {
        self.stages.iter().fold(input, |value, stage| stage(value))
    }
}

/// A pipeline whose stages are built at runtime and may capture state (a
/// calibration offset loaded from config, say) — so each stage has to be a
/// `Box<dyn Fn(f64) -> f64>` rather than a plain function pointer.
#[derive(Default)]
struct DynPipeline {
    stages: Vec<Box<dyn Fn(f64) -> f64>>,
}

impl DynPipeline {
    fn new() -> Self {
        Self::default()
    }

    /// Appends a stage, which may be a closure capturing its environment.
    fn add_stage(&mut self, stage: impl Fn(f64) -> f64 + 'static) {
        self.stages.push(Box::new(stage));
    }

    /// Runs `input` through every stage, left to right.
    fn run(&self, input: f64) -> f64 {
        self.stages.iter().fold(input, |value, stage| stage(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn double(x: f64) -> f64 {
        x * 2.0
    }

    fn add_ten(x: f64) -> f64 {
        x + 10.0
    }

    #[test]
    fn pipeline_runs_stages_left_to_right() {
        let pipeline = Pipeline::new([double, add_ten]);
        // (3 * 2) + 10 = 16, not (3 + 10) * 2 = 26.
        assert_eq!(pipeline.run(3.0), 16.0);
    }

    #[test]
    fn pipeline_of_one_stage() {
        let pipeline = Pipeline::new([double]);
        assert_eq!(pipeline.run(5.0), 10.0);
    }

    #[test]
    fn dyn_pipeline_stages_can_capture_state() {
        let offset = 100.0;
        let mut pipeline = DynPipeline::new();
        pipeline.add_stage(double);
        pipeline.add_stage(move |x| x + offset);
        assert_eq!(pipeline.run(3.0), 106.0);
    }

    #[test]
    fn dyn_pipeline_with_no_stages_is_identity() {
        let pipeline = DynPipeline::new();
        assert_eq!(pipeline.run(42.0), 42.0);
    }
}
