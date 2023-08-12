use std::collections::HashMap;

use colored::*;
use serde::{Deserialize, Serialize};

use commons_error::*;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ParamOption {
    description: String,
    flags: Vec<String>,
    required: bool,
    #[serde(rename = "hasValue")]
    has_value: bool,
    key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Subcommand {
    name: String,
    description: String,
    options: Vec<ParamOption>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Command {
    name: String,
    sub: Vec<Subcommand>,
}

// Help Display

fn display_options(options: &Vec<ParamOption>) {
    for option in options {
        let flag_str = option.flags.join(", ");
        let has_value_str = if option.has_value {
            " <value>".blue().to_string()
        } else {
            "".to_string()
        };
        let required_str = if option.required {
            "(required)".red().to_string()
        } else {
            "".to_string()
        };
        println!("  {}{}{}\t{}",
                 flag_str.green(),
                 has_value_str,
                 required_str,
                 option.description);
    }
}

fn display_subcommands(subcommands: &Vec<Subcommand>) {
    for subcommand in subcommands {
        println!("  {}\t{}", subcommand.name.yellow(), subcommand.description);
        display_options(&subcommand.options);
    }
}

pub(crate) fn display_commands(commands: &[Command]) {
    for command in commands {
        println!("{}", command.name.bold());
        display_subcommands(&command.sub);
        println!();
    }
}


#[derive(Debug)]
pub(crate) struct Params {
    pub(crate) object: String,
    pub(crate) action: String,
    pub(crate) options : HashMap<String, Option<String>>,
}

// fn parse(args : &Vec<String>) -> anyhow::Result<Params> {

pub(crate) fn parse_args(args: &[String]) -> anyhow::Result<Params> {

    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        return   Ok(Params {
            object: "help".to_string(),
            action : "help".to_string(),
            options : HashMap::from([("-h".to_string(), None)]),
        })
    }

    if args.len() == 2 && (args[1] == "-v" || args[1] == "--version") {
        return   Ok(Params {
            object: "version".to_string(),
            action : "version".to_string(),
            options : HashMap::from([("-v".to_string(), None)]),
        })
    }

    // Vérification du nombre d'arguments passés
    if args.len() < 3 {
        return Err(anyhow::anyhow!("Nombre d'arguments insuffisant"));
    }

    // Récupération des arguments obligatoires
    let command = args[1].clone();
    let subcommand = args[2].clone();

    dbg!(&command);
    dbg!(&subcommand);

    // Initialisation de la liste d'options
    let mut options = HashMap::new();

    // Analyse des options
    let mut i = 3;
    while i < args.len() {
        let arg = &args[i];

        dbg!(arg);

        // Si l'argument commence par "-" ou "--", il s'agit d'une option
        if arg.starts_with("-") || arg.starts_with("--") {
            let option_name: String;
            let option_value: Option<String>;

            // Si l'option commence par "--", elle peut avoir une valeur
            if arg.starts_with("--")  || arg.starts_with("-") {
                option_name = arg.clone();
                if i + 1 < args.len() {
                    if args[i + 1].starts_with("--") || args[i + 1].starts_with("-") {
                        option_value = None;
                    } else {
                        option_value = Some(args[i + 1].clone());
                        i += 1;
                    }
                } else {
                    option_value = None;
                }
                // Stockage de l'option dans la liste
                options.insert(option_name, option_value);
            }
        } else {
            return Err(anyhow::anyhow!(format!("Argument inattendu : {}", arg)));
        }

        i += 1;
    }

    Ok(Params {
        object: command,
        action : subcommand,
        options,
    })
}


pub(crate) fn load_commands() -> Vec<Command> {
    let json_str = include_str!("../commands.json");
    serde_json::from_str(json_str).map_err(eprint_fwd!("💣 Problem while loading the list of commands")).unwrap()
}

fn _find_option<'a> (commands: &'a[Command], command_name: &str, subcommand_name: &str, option_flag: &str) -> Result<&'a ParamOption, String> {
    let command = commands.iter()
        .find(|cmd| cmd.name == command_name)
        .ok_or(format!("Command {} not found", command_name))?;

    let subcommand = command.sub.iter()
        .find(|subcmd| subcmd.name == subcommand_name)
        .ok_or(format!("Subcommand {} not found in command {}", subcommand_name, command_name))?;

    let option = subcommand.options.iter()
        .find(|opt| opt.flags.contains(&option_flag.to_string()))
        .ok_or(format!("Option with flag {} not found in subcommand {}", option_flag, subcommand_name))?;

    Ok(option)
}

fn _validate_args<'a>(commands: &'a[Command], args : &[String]) -> Result<&'a ParamOption, String> {
    match parse_args(args) {
        Ok(split_command) => {
            // let (command, subcommand, param_options) = split_command;
            let command = split_command.object;
            let subcommand = split_command.action;
            let param_options = split_command.options;

            for (option_name, option_value) in param_options {
                match _find_option(commands, &command, &subcommand, &option_name ) {
                    Ok(p_option) => {
                        if p_option.has_value && option_value.is_none() {
                            // Err
                        }
                    }
                    Err(_) => {

                    }
                }
            }
        }
        Err(_) => {}
    }

    Err("".to_string())
}


#[cfg(test)]
mod tests {
    use crate::command_options::{Command, find_option, parse_args};

    #[test]
    fn get_data() {
        let json_str = include_str!("../commands.json");
        let commands: Vec<Command> = serde_json::from_str(json_str).unwrap();
        println!("{:#?}", commands);
    }


    #[test]
    fn test_find_options() {
        let json_str = include_str!("../commands.json");
        let commands: Vec<Command> = serde_json::from_str(json_str).unwrap();
        let r = find_option(&commands, "customer", "create", "--name");
        println!("{:#?}", &r);
        let r = find_option(&commands, "customer", "create", "-t");
        println!("{:#?}", &r);
        let r = find_option(&commands, "customer", "crate", "--name");
        println!("{:#?}", &r);
    }

    #[test]
    fn test_parse_args() {
        let my_args = vec![
            "doka-cli".to_string(),
            "customer".to_string(),
            "create".to_string(),
            "--verbose".to_string(),
            "--name".to_string(),
            "customer1".to_string(),
            "-e".to_string(),
            "customer@toto.com".to_string(),
            "-ap".to_string(),
            "secured".to_string(),
        ];
        let result = parse_args(&my_args);
        println!("{:#?}", &result);

        let my_args = vec![
            "doka-cli".to_string(),
            "blink".to_string(),
            "blonck".to_string(),
            "-n".to_string(),
            "-p".to_string(),
            "-c".to_string(),
            "cccc".to_string(),
            "-e".to_string(),
            "eeee".to_string(),
        ];
        let result = parse_args(&my_args);
        println!("{:#?}", &result);

    }
}
