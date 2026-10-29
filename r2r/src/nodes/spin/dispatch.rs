use super::*;

mod actions;
mod ordinary;

/// One stack-local outcome for a readiness snapshot. Independent ready work is dispatched
/// before reporting the first failure; no error list or per-node scheduling state is needed.
#[derive(Default)]
struct DispatchErrors {
    first: Option<Error>,
}

impl DispatchErrors {
    fn error(&mut self, error: Error) {
        self.first.get_or_insert(error);
    }
    fn record(&mut self, result: Result<()>) {
        if let Err(error) = result {
            self.error(error);
        }
    }
    fn result(self) -> Result<()> {
        self.first.map_or(Ok(()), Err)
    }
}

impl WaitEntities<'_> {
    pub(in crate::nodes) fn dispatch(
        &mut self, ws: &rcl_wait_set_t, offsets: EntityOffsets,
    ) -> Result<()> {
        let mut errors = DispatchErrors::default();
        self.dispatch_ordinary(ws, offsets, &mut errors)?;
        self.dispatch_actions(ws, &mut errors);
        errors.result()
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
