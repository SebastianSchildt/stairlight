#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate reqwest;

use std::error::Error;
use std::thread;
use std::time::Duration;



include!("config");



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

fn main() {

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
    thread::sleep(Duration::from_secs(5));
    }

}
