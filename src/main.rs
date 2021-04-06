pub mod textadventure;
use textadventure::*;


extern crate dotenv;
use std::{sync::Arc};
use serenity::{
	async_trait,
	model::{id, channel::Message, gateway::{Ready}},
	prelude::*,
};


//type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub mod logfile;
use logfile::LogFile;


// fn time_snowflake(dt: DateTime<Utc>) -> u64 {
// 	(dt.timestamp_millis() as u64 - 1420070400000) << 22
// }


#[derive(Clone)]
struct Handler {
	text_adventure: Arc<RwLock<Option<TextAdventure>>>,
	text_adventure_channel_id: id::ChannelId,
	logger: LogFile,
}

#[async_trait]
impl EventHandler for Handler {
	async fn message(&self, ctx: Context, msg: Message) {
		if msg.channel_id == self.text_adventure_channel_id && (msg.content.starts_with("\\") || msg.content=="help") && msg.content.len()>1 {
			if let Some(sender)=self.text_adventure.read().await.as_ref().map_or(None,|x|{Some(x.sender.clone())}){
				if let Ok(slot)=sender.try_reserve(){
					if msg.content=="help" {
						self.text_adventure_channel_id.say(&ctx, "Help: Welcome to text adventure bot! Please prefix commands for the adventure with \"\\\\\". \n\
																	A lot of commands are basically English sentences such as \"\\look at the chair\" for example. \n\
																	You can travel in a certain direction by saying a direction such as \"\\east\".\n\
																	Common commands have shortcuts such as \"\\l\" for \"\\look\".").await.ok();
					} else {
						let mut command=String::from("");
						for (i,c) in msg.content.chars().skip(1).enumerate() {
							if i>100 || c.is_ascii_control(){
								break;
							} else if c.is_ascii() && c != '/' && c != '\\' {
								command.push(c);
							}
						}
						if command.len()>0 {
							self.logger.log(&LogFile::format_note(format!("Command: {}", command))).await;
							slot.send(command);
						}
					}
				}
			}
		}
	}
	
	async fn ready(&self, _ctx: Context, _ready: Ready) {
		self.logger.log(&LogFile::format_note("Bot is connected!")).await;
	}
	


	async fn cache_ready(&self, ctx: Context, guilds: Vec<id::GuildId>) {
		if let Some((ta_channel_id,_))=guilds[0].channels(&ctx).await.unwrap().iter().find(|(_key,value)|{value.id==self.text_adventure_channel_id}){
			let mut ta=self.text_adventure.write().await;
			if ta.is_none(){
				ta.replace(TextAdventure::new(ctx.clone(),ta_channel_id.clone(),self.logger.clone()));
			}
		} else {
			self.logger.log(&LogFile::format_error("No text_adventure channel found.")).await;
		}
	}
}

#[tokio::main]
async fn main(){
	dotenv::from_filename("config.env").ok();
	
	let token = std::env::vars().find(|(key,_value)|{key=="DISCORD_TOKEN"}).unwrap().1;
	let ta_channel_id= std::env::vars().find(|(key,_value)|{key=="DISCORD_TA_CHANNEL_ID"}).unwrap().1.parse::<u64>().unwrap();


	
	let mut client = Client::builder(&token)
		.event_handler(Handler {
			text_adventure: Arc::new(RwLock::new(None)),
			text_adventure_channel_id: id::ChannelId(ta_channel_id),
			logger: LogFile::new("botlog.txt"),
		})
		.await
		.expect("Error creating client");

	if let Err(why) = client.start().await {
		eprintln!("Client error: {:?}", why);
	}
}
