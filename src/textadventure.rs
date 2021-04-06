use tokio::time::Duration;
use tokio::{io::{BufReader, AsyncRead, AsyncWriteExt, AsyncBufRead}};
use serenity::client::Context;
use serenity::model::id::ChannelId;
use core::task::Poll;
use std::pin::Pin;
use crate::logfile;
use logfile::LogFile;

//impl AsyncBufRead for BufReader<tokio::process::ChildStdout> { }

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;


struct CustomReader<'a,T>{
	buf_reader: &'a mut T,
}
impl <T: AsyncBufRead> CustomReader<'_,T>{
	fn read(buf_reader: &mut T)->CustomReader<T>{
		CustomReader{buf_reader: buf_reader}
	}
}
impl <T: AsyncBufRead + Unpin> core::future::Future for CustomReader<'_,T>{
	type Output = std::io::Result<Vec<u8>>;
	fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context) -> Poll<Self::Output> {
		let buf_reader=&mut Pin::into_inner(self).buf_reader;
		let (consumed, result) = match tokio::io::AsyncBufRead::poll_fill_buf(Pin::new(buf_reader),cx){
			Poll::Ready(Ok(x)) => (x.len(), Poll::Ready(Ok(Vec::from(x)))),
			Poll::Ready(Err(e)) => (0,Poll::Ready(Err(e))),
			Poll::Pending => (0,Poll::Pending),
		};
		if consumed != 0 {
			tokio::io::AsyncBufRead::consume(Pin::new(buf_reader),consumed);
		}
		return result;
	}
}

async fn read_with_timeout<T: AsyncBufRead + Unpin>(buf_reader: &mut T, duration: Duration) -> Result<AttemptedReadResult> {
	let mut result=String::from("");
	let mut is_output=false;
	while let Ok(buffer_result)=tokio::time::timeout(duration, CustomReader::read(buf_reader)).await{
		let buffer= buffer_result?;
		if buffer.len() !=0 {
			result+=&String::from_utf8_lossy(&buffer);
			is_output=true;
		}else{
			return Ok(AttemptedReadResult::Eof);
		}
	}
	if is_output {
		return Ok(AttemptedReadResult::Something(result));
	} else {
		return Ok(AttemptedReadResult::Nothing);
	}
}

#[derive(PartialEq)]
enum AttemptedReadResult {
	Something(String),
	Nothing,
	Eof,
}


pub struct TextAdventure{
	pub sender: tokio::sync::mpsc::Sender<String>,
	//ctx: Context,
	//channel_id: ChannelId,
	//logger: LogFile,
}

impl TextAdventure{
	pub fn new(ctx: Context,channel_id: ChannelId, logger: LogFile)->TextAdventure{
		let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);
		
		let ctx2=ctx.clone();
		let channel_id2=channel_id.clone();
		let logger2=logger.clone();
		tokio::spawn(async move {
			loop{
				if let Err(e) = TextAdventure::adventure_loop(&mut rx, &ctx2, &channel_id2, &logger2).await{
					logger2.log(&LogFile::format_error(e)).await;
					channel_id2.say(&ctx2, "Error encountered with adventure. Restarting in 5 seconds.").await.ok();
				}
				channel_id2.say(&ctx2, "Adventure ended. Restarting in 10 seconds.").await.ok();
				tokio::time::sleep(Duration::from_millis(10000)).await;
			}
		});
		TextAdventure{
			sender: tx,
			//ctx: ctx,
			//channel_id: channel_id,
			//logger: logger,
			}
	}
	

	
	async fn adventure_loop(rx: &mut tokio::sync::mpsc::Receiver<String>, ctx: &Context,channel_id: &ChannelId, logger: &LogFile)->Result<()>{
		logger.log(&LogFile::format_note("Starting Text Adventure.")).await;
		channel_id.say(&ctx, "STARTING TEXT ADVENTURE").await?;
		let launch_command_env = std::env::vars().find(|(key,_value)|{key=="LAUNCH_COMMAND"}).unwrap().1;
		let split=shell_words::split(&launch_command_env).unwrap();
		if split.len()==0{
			panic!("No command found in environment!");
		}
		let command=&split[0];
		let args:&[String]=if split.len()>1 {&split[1..]} else {&[]};
	
		let mut child = tokio::process::Command::new(command)
						.args(args)
						.stdin(std::process::Stdio::piped())
						.stdout(std::process::Stdio::piped())
						.stderr(std::process::Stdio::piped())
						.spawn()
						.expect("failed to execute child");
						
		let stdout = child.stdout.take().expect("no stdout");
		let stderr = child.stderr.take().expect("no stderr");
		let mut stdin = child.stdin.take().expect("no stdout");
		let mut buf_reader_stdout = BufReader::new(stdout);
		let mut buf_reader_stderr = BufReader::new(stderr);
		let mut is_eof=false;
		let mut send_to_discord=String::from("");
		
		while !is_eof{
			
			let from_stdout=read_with_timeout(&mut buf_reader_stdout,Duration::from_millis(200)).await?;
			if from_stdout == AttemptedReadResult::Eof { is_eof=true; }
			
			match read_with_timeout(&mut buf_reader_stderr,Duration::from_millis(100)).await? {
				AttemptedReadResult::Something(mut result) => {
						let trimmed=result.trim();
						let mut truncated=String::from("");
						for (i,c) in trimmed.chars().enumerate() {
							truncated.push(if c=='\n' {'~'} else {c});
							if i>100 {break;}
						}
						logger.log(&LogFile::format_error(format!("{}", truncated))).await;
					},
				AttemptedReadResult::Nothing => {},
				AttemptedReadResult::Eof => { is_eof=true; },
			}
			
			if let AttemptedReadResult::Something(result)=from_stdout {
				{
					let trimmed=send_to_discord + &result.trim();
					let mut truncated=String::from("");
					for (i,c) in trimmed.chars().enumerate() {
						truncated.push(if c=='\n' {'~'} else {c});
						if i>100 {break;}
					}
					logger.log(&LogFile::format_note(format!("Result: {}", truncated))).await;
					if trimmed != "" {
						channel_id.say(&ctx, trimmed).await?;
					}
				}
				if let Some(message) = rx.recv().await {
					send_to_discord = format!("<< {} >>\n",message);
					stdin.write_all(format!("{}\n",message).as_bytes()).await?;
				} else {break;}
			}
		}
		//println!("HERE!!");
		Ok(())
	}
	
}
