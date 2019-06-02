//#[macro_use]
extern crate reqwest;
extern crate serde;
extern crate serde_derive;

#[macro_use]
extern crate clap;
use clap::App;

#[macro_use]
extern crate log;
extern crate simplelog;

use simplelog::*;

use std::collections::HashMap;
use std::error::Error;
use std::thread;
use std::time::Duration;

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

struct Config {
    hue: String,
    huetoken: String,
    espurna: String,
    espurnatoken: String,
    master: u32,
}

fn check_master_light(huestate: &mut LightState, config: &Config) -> Result<bool, Box<dyn Error>> {
    let request_url = format!(
        "http://{hue}/api/{user}/lights/{master}",
        hue = config.hue,
        user = config.huetoken,
        master = config.master
    );
    debug!("check_master_light: request_url: {}", request_url);

    let mut response = reqwest::get(&request_url)?;
    let jsonstr = response.text()?;
    debug!("check_master_light: respsonse {}", jsonstr);

    let v: serde_json::Value = serde_json::from_str(&jsonstr)?;
    let state: String = v["state"]["on"].to_string();

    //If we can not get color config, return white light, 50% bright
    let mut h: u64 = v["state"]["hue"].as_u64().unwrap_or(0);
    let mut s: u64 = v["state"]["sat"].as_u64().unwrap_or(0);
    let mut v: u64 = v["state"]["bri"].as_u64().unwrap_or(127);

    debug!("Master light config H/S/V: {}/{}/{}", h, s, v);

    /* Hue       h: 0...65535, s: 0...255, v 0...255
     * Espurna   h: 0...360,   s: 0...100, v 0...100
     * convert to Espurna, as it is more coarse, and can't track smaller changes anyway
     */
    h = (h as f64 / 65535.0 * 360.0) as u64;
    s = (s as f64 / 255.0 * 100.0) as u64;
    v = (v as f64 / 255.0 * 100.0) as u64;
    debug!("Espurna-fied H/S/V: {}/{}/{}", h, s, v);

    huestate.h = h;
    huestate.s = s;
    huestate.v = v;

    if state == "true" {
        //println!("Is on");
        huestate.state = State::On;
        Ok(true)
    } else {
        //println!("Is off or broken");
        huestate.state = State::Off;
        Ok(false)
    }
}

fn switch_slave_light(
    masterstate: &LightState,
    currstate: &mut LightState,
    config: &Config,
) -> Result<(), Box<dyn Error>> {
    if masterstate != currstate {
        info!("State change detected. Syncing Espurna....");
    } else {
        debug!("No change, not pestering slave light");
        return Ok(());
    }

    let token: &str = &config.espurnatoken;

    let client = reqwest::Client::new();

    let request_url = format!("http://{espurna}/api/hsv", espurna = config.espurna);
    let mut formdata = HashMap::new();
    formdata.insert("apikey", token);
    let hsvstr: &str = &format!("{},{},{}", masterstate.h, masterstate.s, masterstate.v);
    formdata.insert("value", hsvstr);
    let mut response = client
        .request(reqwest::Method::PUT, &request_url)
        .header("Accept", "application/json")
        .form(&formdata)
        .send()?;

    let jsonstr = response.text()?;
    debug!("switch_slave_light: Got answer setting hsv: {}", jsonstr);

    let client = reqwest::Client::new();

    let request_url = format!("http://{espurna}/api/relay/0", espurna = config.espurna);
    debug!("switch_slave_light: Uri for relay {}", request_url);

    let mut formdata = HashMap::new();
    formdata.insert("apikey", token);
    if masterstate.state == State::On {
        debug!("switch_slave_light: Switching Espurna ON");
        formdata.insert("value", "1");
    } else {
        debug!("switch_slave_light: Switching Espurna OFF");
        formdata.insert("value", "0");
    }
    let mut response = client
        .request(reqwest::Method::PUT, &request_url)
        .header("Accept", "application/json")
        .form(&formdata)
        .send()?;

    let jsonstr = response.text()?;
    debug!("switch_slave_light: Got answer setting state: {}", jsonstr);

    currstate.state = masterstate.state.clone();
    currstate.h = masterstate.h;
    currstate.s = masterstate.s;
    currstate.v = masterstate.v;

    Ok(())
}

fn main() {
    let mut esp_state = LightState {
        state: State::Unknown,
        h: 0,
        s: 0,
        v: 0,
    };
    let mut hue_state = LightState {
        state: State::Unknown,
        h: 0,
        s: 0,
        v: 0,
    };

    let mut myconfig = Config {
        hue: String::from("Unknown"),
        espurna: String::from("Unknown"),
        huetoken: String::from("0"),
        espurnatoken: String::from("0"),
        master: 0,
    };

    //parse cmdline
    let yaml = load_yaml!("cmdline.yaml");
    let matches = App::from_yaml(yaml).get_matches();

    myconfig.hue = String::from(matches.value_of("huebridge").unwrap());
    myconfig.huetoken = String::from(matches.value_of("huetoken").unwrap());
    myconfig.espurna = String::from(matches.value_of("espurna").unwrap());
    myconfig.espurnatoken = String::from(matches.value_of("espurnatoken").unwrap());
    myconfig.master = value_t!(matches.value_of("masterlight"), u32).unwrap_or_else(|e| e.exit());
    let debug = matches.is_present("debug");

    let mut level = LevelFilter::Info;
    if debug {
        level = LevelFilter::Debug;
    }

    CombinedLogger::init(vec![
        TermLogger::new(level, simplelog::Config::default()).unwrap()
    ])
    .unwrap();

    info!("Will connect to Hue at......: {}", myconfig.hue);
    debug!("Will use Hue token.........: {}", myconfig.huetoken);
    info!("Will connect to Espurna at..: {}", myconfig.espurna);
    debug!("Will use Espurna token.....: {}", myconfig.espurnatoken);
    info!("Will track light............: {}", myconfig.master);

    loop {
        debug!("Checking state....");

        let res = check_master_light(&mut hue_state, &myconfig);

        let res = match res {
            Ok(res) => res,
            Err(error) => {
                error!("There was a problem checking master light: {:?}", error);
                false
            }
        };

        info!("Is light on? {}", res);

        let res2 = switch_slave_light(&hue_state, &mut esp_state, &myconfig);
        match res2 {
            Ok(()) => {}
            Err(error) => {
                error!("There was a problem switching slave light: {:?}", error);
            }
        };

        thread::sleep(Duration::from_secs(2));
    }
}
