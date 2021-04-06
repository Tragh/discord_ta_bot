use chrono::Utc;

#[derive(Clone)]
pub struct LogFile {
	sender: tokio::sync::mpsc::Sender<String>,
}
impl LogFile {
	pub fn format_note<S>(s: S)->String where S: std::fmt::Display{
		format!("[{}] {}",Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),s)
	}
	pub fn format_error<S>(s: S)->String where S: std::fmt::Debug{
		format!("[{}] ERROR {:?}",Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),s)
	}
	pub async fn log(&self,s: &String){
		self.sender.send(s.clone()).await.ok();
	}
	pub fn new<S>(filename: S) -> LogFile where S: Into<String>{
		use tokio::sync::mpsc;
		use std::io::Write;
		let filename=Into::<String>::into(filename);
		let (tx, mut rx) = mpsc::channel::<String>(8);
		tokio::spawn(async move {
			let mut file=std::fs::OpenOptions::new().append(true).create(true).open(filename).unwrap();
			while let Some(message) = rx.recv().await {
				println!("{}",message);
				writeln!(file,"{}",message).ok();
				file.flush().ok();
			}
		});
		LogFile{sender: tx}
	}
}
