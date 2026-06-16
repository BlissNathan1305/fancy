fn main() {
    // Create a vector containing a list of American cities
    let cities = vec![
        "New York City",
        "Los Angeles",
        "Chicago",
        "Houston",
        "Phoenix",
        "Philadelphia",
        "San Antonio",
        "San Diego",
        "Dallas",
        "San Jose",
    ];

    println!("--- List of American Cities ---");

    // Iterate through the vector and print each city
    for city in &cities {
        println!("{}", city);
    }
}
