fn main() {
    println!("Calling open::that...");
    let res = open::that("http://example.com");
    println!("Result: {:?}", res);
}
