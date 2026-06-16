use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

// This attribute tells Serde how to convert this struct to and from JSON
#[derive(Serialize, Deserialize, Debug)]
struct TodoItem {
    title: String,
    completed: bool,
}

const FILE_PATH: &str = "todo.json";

fn main() {
    // 1. Load existing todos or start with an empty list
    let mut todo_list: Vec<TodoItem> = load_todos(FILE_PATH).unwrap_or_else(|_| Vec::new());

    loop {
        println!("\n--- Rust Todo List ---");
        println!("1. View Todos");
        println!("2. Add Todo");
        println!("3. Complete Todo");
        println!("4. Exit");
        print!("Choose an option: ");
        io::stdout().flush().unwrap(); // Ensure the prompt prints immediately

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).expect("Failed to read line");
        
        // Match user input safely
        match choice.trim() {
            "1" => view_todos(&todo_list),
            "2" => add_todo(&mut todo_list),
            "3" => complete_todo(&mut todo_list),
            "4" => {
                // Save before exiting
                if save_todos(FILE_PATH, &todo_list).is_ok() {
                    println!("Todos saved successfully. Goodbye!");
                } else {
                    eprintln!("Error saving todos before exit.");
                }
                break;
            }
            _ => println!("Invalid option, please try again."),
        }
    }
}

// Display all items in the vector
fn view_todos(list: &Vec<TodoItem>) {
    if list.is_empty() {
        println!("\nYour todo list is empty!");
        return;
    }

    println!("\nYour Tasks:");
    for (index, item) in list.iter().enumerate() {
        let status = if item.completed { "[X]" } else { "[ ]" };
        println!("{} {} - {}", index + 1, status, item.title);
    }
}

// Add a new item to the vector
fn add_todo(list: &mut Vec<TodoItem>) {
    print!("\nEnter task title: ");
    io::stdout().flush().unwrap();

    let mut title = String::new();
    io::stdin().read_line(&mut title).expect("Failed to read line");
    let title = title.trim().to_string();

    if !title.is_empty() {
        list.push(TodoItem { title, completed: false });
        println!("Task added!");
    } else {
        println!("Task title cannot be empty.");
    }
}

// Mark an item as completed based on its index
fn complete_todo(list: &mut Vec<TodoItem>) {
    if list.is_empty() {
        println!("\nNo tasks to complete.");
        return;
    }

    view_todos(list);
    print!("\nEnter the number of the task to complete: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Failed to read line");

    // Parse string input into a number index
    if let Ok(num) = input.trim().parse::<usize>() {
        if num > 0 && num <= list.len() {
            list[num - 1].completed = true;
            println!("Task marked as completed!");
            return;
        }
    }
    println!("Invalid task number.");
}

// Helper: Load JSON file back into Rust structs
fn load_todos<P: AsRef<Path>>(path: P) -> Result<Vec<TodoItem>, io::Error> {
    let file = File::open(path)?;
    let list = serde_json::from_reader(file)?;
    Ok(list)
}

// Helper: Save Rust structs into a JSON file
fn save_todos<P: AsRef<Path>>(path: P, list: &Vec<TodoItem>) -> Result<(), io::Error> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, list)?;
    Ok(())
}
