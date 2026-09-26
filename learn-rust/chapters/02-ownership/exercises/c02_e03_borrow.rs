// c02_e03_borrow.rs
//
// CONCEPT: shared borrows `&T` — reading without taking ownership.
// GUIDE:   chapters/02-ownership/guide.md  (section "Borrowing: lending instead of giving")
// DATABROKER: databroker/src/broker.rs — `Entry::validate(&self, update:
//             &EntryUpdate)` only *looks* at the update, so it borrows it.
// EXPECT:  compile errors E0382 (use of moved value: `readings`) and E0308
//          (the tests already pass slices like `&[4, 6]`).
//
// If a function only needs to *read* data, it should borrow it: `&T`.
// The caller keeps ownership and can keep using the value afterwards.
// Any number of shared borrows may exist at the same time.
//
// For a list of numbers, borrow it as a *slice* `&[u32]` rather than
// `&Vec<u32>`: a `&Vec<u32>` converts to `&[u32]` automatically, and a slice
// parameter also accepts arrays and parts of vectors. (Clippy warns about
// `&Vec<T>` parameters for exactly this reason.)
//
// YOUR TASK
//   Change `average` and `maximum` so they borrow the readings instead of
//   taking ownership, and update the calls in `report` to pass `&readings`.
//   Do not edit the tests.

// I AM NOT DONE

fn average(readings: Vec<u32>) -> u32 {
    let mut total = 0;
    for reading in readings {
        total += reading;
    }
    total / readings.len() as u32
}

fn maximum(readings: Vec<u32>) -> u32 {
    let mut max = 0;
    for reading in readings {
        if reading > max {
            max = reading;
        }
    }
    max
}

/// Returns (average, maximum, number of readings).
fn report(readings: Vec<u32>) -> (u32, u32, usize) {
    let avg = average(readings);
    let max = maximum(readings);
    (avg, max, readings.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_works() {
        assert_eq!(report(vec![10, 20, 60]), (30, 60, 3));
    }

    #[test]
    fn slices_accept_arrays_too() {
        assert_eq!(average(&[4, 6]), 5);
        assert_eq!(maximum(&[4, 6, 1]), 6);
    }
}
