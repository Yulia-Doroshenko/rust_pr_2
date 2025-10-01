mod part_1;
mod part_2;

use std::fs;

fn main() {
    let js = fs::read_to_string("./request.json").expect("request.json");
    let toml = part_2::json_to_toml(&js).expect("json->toml");
    println!("{toml}");
}