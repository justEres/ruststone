use rs_utils::{ToMainMessage, ToNetMessage};





pub fn start_networking(from_main: crossbeam::channel::Receiver<ToNetMessage>, to_main: crossbeam::channel::Sender<ToMainMessage>){
    



    while let Ok(msg) = from_main.recv(){
        match msg{
            ToNetMessage::Connect{username,address}=>{
                println!("Connecting to server at {} as {}",address,username);

            }
            _ => {
                println!("Received unhandled ToNetMessage");
            }
        }
    }
}