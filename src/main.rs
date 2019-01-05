#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate reqwest;

use std::collections::HashMap;
use std::error::Error;
use std::thread;
use std::time::Duration;



include!("config");

#[derive(PartialEq)]
enum State {
    On,
    Off,
    Unknown,
}

struct EspurnaState {
    state: State,
    color: String,
}

#[derive(Deserialize, Debug)]
struct HueState {
    on: bool,
}

#[derive(Deserialize, Debug)]
struct HueJson {
    state: HueState,
}


fn check_master_light() -> Result<bool,Box<dyn Error>> {
    let request_url = format!("http://{hue}/api/{user}/lights/{master}",
                              hue = HUE_LOCATION,
                              user = HUE_USER,
                              master = HUE_MASTER_LIGHT);
    println!("{}", request_url);

    let mut response = reqwest::get(&request_url)?;
    let  jsonstr = response.text()?;
    //println!("{}", test);

    println!("Parse");
    let v: serde_json::Value = serde_json::from_str(&jsonstr)?;
    let  state : String = v["state"]["on"].to_string();
    
    if  state == "true"  {
        //println!("Is on");
        Ok(true)
    }
    else {
        //println!("Is off or broken");
        Ok(false)
    }
}

fn switch_slave_light(newstate: bool, currstate: &EspurnaState)  -> Result<(),Box<dyn Error>> {

    let ns = match newstate {
        true => State::On,
        false => State::Off,
    };

    if ns == currstate.state {
        println!("No change, not pestering slave light");
        return Ok(())
    }
    let client = reqwest::Client::new();

    let request_url = format!("http://{espurna}/api/relay/0",
                              espurna = ESPURNA_LOCATION);
    println!("{}", request_url);

    let mut formdata = HashMap::new();
    formdata.insert("apikey", ESPURNA_APIKEY);
    if newstate {
        println!("Switching Espurna ON");
        formdata.insert("value", "1");
    }
    else {
        println!("Switching Espurna OFF");
        formdata.insert("value", "0");
    }


   
    let mut response = client.request(reqwest::Method::PUT,&request_url).header("Accept","application/json").form(&formdata).send()?;

    let  jsonstr = response.text()?;
    println!("Got answer: {}", jsonstr);

    let v: serde_json::Value = serde_json::from_str(&jsonstr)?;
    let  state : String = v["relay/0"].to_string();

    println!("Espurna state is {}",state);
    Ok(())
}

fn main() {

    let mut state = EspurnaState { state: State::Unknown, color: String::from("")};

    loop {
            println!("Checking....");

    let res=check_master_light();

    let res = match res {
        Ok(res) => res,
        Err(error) => {
            println!("There was a problem checking master light: {:?}", error);
            false
        },
    };

    println!("Is light on? {}",res);

    let res2 = switch_slave_light(res, &state);
    let res2 = match res2 {
        Ok(res2) => { 
            match res {
                true => { state.state = State::On }
                false => { state.state = State::Off}
            }            
        }
        Err(error) => {
            println!("There was a problem switching slave light: {:?}", error);
        },
    };

    thread::sleep(Duration::from_secs(2));
    }

}
