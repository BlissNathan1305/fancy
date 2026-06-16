struct Confederation {
    name: &'static str,
    acronym: &'static str,
    teams: Vec<&'static str>,
}

fn main() {
    let world_cup_teams = vec![
        Confederation {
            name: "North, Central America and Caribbean",
            acronym: "CONCACAF",
            teams: vec![
                "Canada (Co-host)", "Mexico (Co-host)", "United States (Co-host)", 
                "Curaçao", "Haiti", "Panama"
            ],
        },
        Confederation {
            name: "Europe",
            acronym: "UEFA",
            teams: vec![
                "Austria", "Belgium", "Bosnia and Herzegovina", "Croatia", "Czechia", 
                "England", "France", "Germany", "Netherlands", "Norway", 
                "Portugal", "Scotland", "Spain", "Sweden", "Switzerland", "Türkiye"
            ],
        },
        Confederation {
            name: "South America",
            acronym: "CONMEBOL",
            teams: vec!["Argentina", "Brazil", "Colombia", "Ecuador", "Paraguay", "Uruguay"],
        },
        Confederation {
            name: "Africa",
            acronym: "CAF",
            teams: vec![
                "Algeria", "Cabo Verde", "DR Congo", "Egypt", "Ghana", 
                "Ivory Coast", "Morocco", "Senegal", "South Africa", "Tunisia"
            ],
        },
        Confederation {
            name: "Asia",
            acronym: "AFC",
            teams: vec![
                "Australia", "Iran", "Iraq", "Japan", "Jordan", 
                "Qatar", "Saudi Arabia", "South Korea", "Uzbekistan"
            ],
        },
        Confederation {
            name: "Oceania",
            acronym: "OFC",
            teams: vec!["New Zealand"],
        },
    ];

    let mut total_count = 0;
    println!("====================================================");
    println!("      OFFICIAL FIFA WORLD CUP 48-TEAM FIELD        ");
    println!("====================================================\n");

    for confed in &world_cup_teams {
        println!("## {} ({}) - {} Teams", confed.name, confed.acronym, confed.teams.len());
        println!("----------------------------------------------------");
        
        for (i, team) in confed.teams.iter().enumerate() {
            println!(" {:2}. {}", i + 1, team);
            total_count += 1;
        }
        println!(); // Blank line between confederations
    }

    println!("====================================================");
    println!(" Total Qualified Nations Printed: {}/48", total_count);
    println!("====================================================");
}
