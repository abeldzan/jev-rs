use typesafe_sdk::{Choice, Noul, blocking::Client};

fn main() -> Result<(), typesafe_sdk::Error> {
    let client = Client::from_env()?;
    let response = client
        .system_one("I was charged twice. Please help ASAP.")
        .question("billing", Noul::new("Is this about billing?"))
        .question(
            "tone",
            Choice::new("What is the tone?").options(["calm", "angry"]),
        )
        .send()?;
    println!("billing={:?}", response.noul("billing"));
    println!("tone={:?}", response.choice("tone"));
    Ok(())
}
