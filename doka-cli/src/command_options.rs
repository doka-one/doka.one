use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use crate::Params;

#[derive(Serialize, Deserialize, Debug)]
struct Option {
    description: String,
    flags: Vec<String>,
    required: bool,
    #[serde(rename = "hasValue")]
    has_value: bool,
    key: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Subcommand {
    name: String,
    description: String,
    options: Vec<Option>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Command {
    name: String,
    sub: Vec<Subcommand>,
}

fn parse(args : &Vec<String>) -> anyhow::Result<Params> {

    if args.len() < 3 {
        return Err(anyhow!("Not enough arguments"));
    }

    let command_str = include_str!("../commands.json");
    let commands: Vec<Command> = serde_json::from_str(command_str).unwrap();

    let object = args.get(1).ok_or(anyhow!("Don't find 1st param"))?.clone();
    let action = args.get(2).ok_or(anyhow!("Don't find 2nd param"))?.clone();
    let mut options : Vec<(String, String)> = vec![];
    let mut i = 3;

    loop {
        if i > args.len()-1 {
            break;
        }
        let option_name = args.get(i).ok_or(anyhow!("Don't find param, i=[{}]", i))?.clone();
        i+=1;

        let o_option = find_option(&commands, &object, &action, &option_name);

        let option_value = match o_option {
            Ok(option) => {
                if option.has_value {
                    args.get(i).ok_or(anyhow!("Don't find param, i+1=[{}]", i+1))?.clone();
                    i += 1
                }
            }
            Err(_) => {}
        };

        options.push((option_name, option_value));

    }

    Ok(Params {
        object,
        action,
        options,
    })
}


fn find_option(commands: &[Command], command_name: &str, subcommand_name: &str, option_flag: &str) -> Result<&Option, String> {
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



#[cfg(test)]
mod tests {
    use crate::command_options::Command;

    #[test]
    fn get_data() {
        let json_str = include_str!("../commands.json");

        let commands: Vec<Command> = serde_json::from_str(json_str).unwrap();
        println!("{:#?}", commands);
    }
}
