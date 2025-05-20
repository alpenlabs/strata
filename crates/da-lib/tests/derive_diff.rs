use strata_da_lib::DaDiff;

// Define dummy types for custom diffs
#[derive(Debug, Clone)]
pub struct IntDiff;

#[derive(Debug, Clone)]
pub struct VecDiff;

#[derive(DaDiff)]
pub struct TestStruct {
    #[diff(IntDiff)]
    pub a: u64,

    #[diff(VecDiff)]
    pub b: Vec<String>,
}

#[derive(DaDiff)]
pub struct NestedStruct {
    #[diff(TestStructDiff)]
    pub test: TestStruct,
}

#[test]
fn test_macro_generates_diff_struct() {
    let tdiff = TestStructDiff {
        a_diff: vec![IntDiff],
        b_diff: vec![VecDiff],
    };

    let _ = NestedStructDiff {
        test_diff: vec![tdiff],
    };
}
