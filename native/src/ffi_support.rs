use std::cell::RefCell;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub fn set_last_error(error: String) {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = Some(error));
}

pub fn clear_last_error() {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = None);
}

pub fn last_error() -> Option<String> {
    LAST_ERROR.with(|cell| cell.borrow().clone())
}

pub fn guarded<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(error) => Err(format!(
            "native panic: {}",
            error
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| error.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic")
        )),
    }
}
