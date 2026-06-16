struct WorldCupFormat {
    era: &'static str,
    number_of_nations: u32,
    number_of_matches: u32,
}

fn main() {
    // Defining the historical and expanded World Cup configurations
    let classic_format = WorldCupFormat {
        era: "1998 - 2022",
        number_of_nations: 32,
        number_of_matches: 64,
    };

    let expanded_format = WorldCupFormat {
        era: "2026 - Present",
        number_of_nations: 48,
        number_of_matches: 104,
    };

    println!("=========================================");
    println!("   FIFA WORLD CUP PARTICIPATING NATIONS   ");
    println!("=========================================");
    
    print_format_details(&classic_format);
    println!("-----------------------------------------");
    print_format_details(&expanded_format);
    
    println!("=========================================");
}

fn print_format_details(format: &WorldCupFormat) {
    println!("Era:                  {}", format.era);
    println!("Nations Qualified:    {} teams", format.number_of_nations);
    println!("Total Matches Played: {} games", format.number_of_matches);
}
