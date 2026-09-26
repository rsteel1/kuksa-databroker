// c20_e02_tokio_spawn_joinhandle.rs
//
// CONCEPT: `tokio::spawn` runs a future as an independent task; it requires
//          the future to be `Send + 'static`, so it must OWN what it uses.
// GUIDE:   chapters/20-async-basics/guide.md  (section "Tasks: tokio::spawn and JoinHandle")
// DATABROKER: databroker/src/grpc/kuksa_val_v2/val.rs — `open_provider_stream`
//             does `let broker = self.clone();` and then
//             `tokio::spawn(async move { ... })`
// EXPECT:  compile errors E0597 (`names` does not live long enough) and E0373
//          (async block may outlive the current function, but it borrows `name`)
//
// A spawned task may outlive the function that spawned it, so the compiler
// forbids it from borrowing local variables. The fix used all over the
// databroker: move owned data (or cheap `Arc`/`Clone` handles) into an
// `async move` block.
//
// YOUR TASK
//   Make `greet_all` compile. It must spawn one task per name and return the
//   greetings in the SAME order as the input.
//   Do not edit the tests.

// I AM NOT DONE

use tokio::task::JoinHandle;

async fn greet_all(names: Vec<String>) -> Vec<String> {
    let mut handles: Vec<JoinHandle<String>> = Vec::new();
    for name in &names {
        handles.push(tokio::spawn(async { format!("hello {name}") }));
    }

    let mut greetings = Vec::with_capacity(handles.len());
    for handle in handles {
        // `JoinHandle` is itself a future. It resolves to `Result<T, JoinError>`
        // — `Err` if the task panicked or was aborted.
        greetings.push(handle.await.expect("greeter task panicked"));
    }
    greetings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn greets_in_order() {
        let names = vec!["speed".to_string(), "rpm".to_string(), "gear".to_string()];
        assert_eq!(
            greet_all(names).await,
            vec!["hello speed", "hello rpm", "hello gear"]
        );
    }

    #[tokio::test]
    async fn empty_input() {
        assert!(greet_all(Vec::new()).await.is_empty());
    }

    #[tokio::test]
    async fn panics_surface_as_join_errors() {
        let handle = tokio::spawn(async { panic!("provider crashed") });
        let result: Result<(), _> = handle.await;
        assert!(result.unwrap_err().is_panic());
    }
}
