fn main() {
    // Create a vector containing a list of EU countries
    let eu_countries = vec![
        "Austria", "Belgium", "Bulgaria", "Croatia", "Republic of Cyprus",
        "Czech Republic", "Denmark", "Estonia", "Finland", "France",
        "Germany", "Greece", "Hungary", "Ireland", "Italy",
        "Latvia", "Lithuania", "Luxembourg", "Malta", "Netherlands",
        "Poland", "Portugal", "Romania", "Slovakia", "Slovenia",
        "Spain", "Sweden",
    ];

    println!("--- Member States of the European Union ({}) ---", eu_countries.len());

    // Iterate with an index, starting the count at 1
    for (index, country) in eu_countries.iter().enumerate() {
        println!("{}. {}", index + 1, country);
    }
}
