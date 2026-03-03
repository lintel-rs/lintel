use combine_structs::combine_fields;

#[combine_fields(DoesNotExist)]
#[derive(Debug)]
pub struct Target {
    pub x: f64,
}

fn main() {}
