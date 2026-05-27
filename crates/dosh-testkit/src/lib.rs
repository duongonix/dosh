use anyhow::Result;

pub fn assert_ok<T>(result: Result<T>) {
    assert!(result.is_ok());
}
