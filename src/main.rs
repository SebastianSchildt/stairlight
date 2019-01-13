//#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate reqwest;

use std::collections::HashMap;
use std::error::Error;
use std::thread;
use std::time::Duration;



include!("config");

#[derive(PartialEq, Copy, Clone)]
enum State {
    On,
    Off,
    Unknown,
}

#[derive(PartialEq)]
struct LightState {
    state: State,
    h: u64,
    s: u64,
    v: u64,
}


fn check_master_light(huestate: &mut LightState) -> Result<bool,Box<dyn Error>> {
    let request_url = format!("http://{hue}/api/{user}/lights/{master}",
                              hue = HUE_LOCATION,
                              user = HUE_USER,
                              master = HUE_MASTER_LIGHT);
    println!("{}", request_url);

    let mut response = reqwest::get(&request_url)?;
    let  jsonstr = response.text()?;
    println!("{}", jsonstr);

    println!("Parse");
    let v: serde_json::Value = serde_json::from_str(&jsonstr)?;
    let  state : String = v["state"]["on"].to_string();



    //If we can not get color config, return white light, 50% bright
    let mut h : u64 = v["state"]["hue"].as_u64().unwrap_or(0);
    let mut s : u64 = v["state"]["sat"].as_u64().unwrap_or(0);
    let mut v : u64 = v["state"]["bri"].as_u64().unwrap_or(127);

    println!("Master light config H/S/V: {}/{}/{}",h,s,v);

    /* Hue       h: 0...65535, s: 0...255, v 0...255
     * Espurna   h: 0...360,   s: 0...100, v 0...100 
     * convert to Espurna, as it is more coarse, and can't track smaller changes anyway
     */
    h = (h as f64/65535.0*360.0) as u64;
    s = (s as f64/255.0*100.0)   as u64;
    v = (v as f64 /255.0*100.0)  as u64;
    println!("Espurna-fied H/S/V: {}/{}/{}",h,s,v);

    huestate.h = h;
    huestate.s = s;
    huestate.v = v;

    if  state == "true"  {
        //println!("Is on");
        huestate.state=State::On;
        Ok(true)
    }
    else {
        //println!("Is off or broken");
        huestate.state=State::Off;
        Ok(false)
    }
}

fn switch_slave_light(masterstate: &LightState, currstate: &mut LightState)  -> Result<(),Box<dyn Error>> {

    if masterstate != currstate {
        println!("State change detected. Syncing Espurna....");
    }
    else {
        println!("No change, not pestering slave light");
        return Ok(())
    }

    let client = reqwest::Client::new();

    let request_url = format!("http://{espurna}/api/hsv",
                              espurna = ESPURNA_LOCATION);
    
    let mut formdata = HashMap::new();
    formdata.insert("apikey", ESPURNA_APIKEY);
    let hsvstr: &str = &format!("{},{},{}",masterstate.h, masterstate.s, masterstate.v);
    formdata.insert("value", hsvstr);
    let mut response = client.request(reqwest::Method::PUT,&request_url).header("Accept","application/json").form(&formdata).send()?;

    let  jsonstr = response.text()?;
    println!("Got answer setting hsv: {}", jsonstr);


    let client = reqwest::Client::new();

    let request_url = format!("http://{espurna}/api/relay/0",
                              espurna = ESPURNA_LOCATION);
    println!("{}", request_url);

    let mut formdata = HashMap::new();
    formdata.insert("apikey", ESPURNA_APIKEY);
    if masterstate.state == State::On {
        println!("Switching Espurna ON");
        formdata.insert("value", "1");
    }
    else {
        println!("Switching Espurna OFF");
        formdata.insert("value", "0");
    }
    let mut response = client.request(reqwest::Method::PUT,&request_url).header("Accept","application/json").form(&formdata).send()?;

    let  jsonstr = response.text()?;
    println!("Got answer setting state: {}", jsonstr);

    currstate.state = masterstate.state.clone();
    currstate.h     = masterstate.h;
    currstate.s     = masterstate.s;
    currstate.v     = masterstate.v;

    Ok(())
}

fn main() {

    let mut esp_state = LightState { state: State::Unknown, h:0, s:0, v:0 };
    let mut hue_state = LightState { state: State::Unknown, h:0, s:0, v:0 };

    loop {
            println!("Checking....");

    let res=check_master_light(&mut hue_state);

    let res = match res {
        Ok(res) => res,
        Err(error) => {
            println!("There was a problem checking master light: {:?}", error);
            false
        },
    };

    println!("Is light on? {}",res);


    let res2 = switch_slave_light(&hue_state, &mut esp_state);
    match res2 {
        Ok(()) => { 
          
        }
        Err(error) => {
            println!("There was a problem switching slave light: {:?}", error);
        },
    };

    thread::sleep(Duration::from_secs(2));
    }

}
