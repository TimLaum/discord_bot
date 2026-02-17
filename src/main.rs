use poise::serenity_prelude as serenity;
use dotenvy::dotenv;
use std::env;
use songbird::SerenityInit;
use reqwest::Client as HttpClient;
use songbird::input::YoutubeDl;
use songbird::tracks::TrackState;
use std::sync::Arc;
use rand::seq::SliceRandom;
use tokio::process::Command;
use futures::stream::{self, StreamExt};


struct Data {}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command, guild_only)]
async fn play(
    ctx: Context<'_>,
    #[description = "Lien YouTube, Spotify ou SoundCloud"] url: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let channel_id = {
        let guild = ctx.guild().unwrap();
        guild.voice_states.get(&ctx.author().id)
            .and_then(|voice_state| voice_state.channel_id)
    };

    let manager = songbird::get(ctx.serenity_context()).await
        .expect("Songbird non initialisé").clone();

    let handler_lock = if let Some(handler) = manager.get(guild_id) {
        handler
    } else {
        let connect_to = match channel_id {
            Some(c) => c,
            None => {
                ctx.say("❌ Tu dois être dans un salon vocal !").await?;
                return Ok(());
            }
        };
        match manager.join(guild_id, connect_to).await {
            Ok(handler) => handler,
            Err(e) => {
                ctx.say(format!("❌ Erreur de connexion: {:?}", e)).await?;
                return Ok(());
            }
        }
    };

    let mut handler = handler_lock.lock().await;

    let http_client = HttpClient::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    let src = YoutubeDl::new(http_client, url);

    handler.enqueue_input(src.into()).await;

    let queue_len = handler.queue().len();
    if queue_len > 1 {
        ctx.say(format!("📥 Ajouté à la file (Position : {})", queue_len - 1)).await?;
    } else {
        ctx.say("▶️ Musique lancée !").await?;
    }

    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn queue(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue().current_queue();

        if queue.is_empty() {
            ctx.say("📭 La file d'attente est vide.").await?;
            return Ok(());
        }

        let mut response = String::from("📑 **File d'attente :**\n");

        for (i, track) in queue.iter().enumerate() {
            let data: Arc<TrackState> = track.data();
            // Songbird TrackState does not have metadata/title, so show play_time or a placeholder
            let info = format!("Durée jouée: {:?}", data.play_time);
            if i == 0 {
                response.push_str(&format!("▶️ **En cours :** {}\n", info));
            } else {
                response.push_str(&format!("**{}**. {}\n", i, info));
            }
        }

        ctx.say(response).await?;
    } else {
        ctx.say("❌ Je ne joue aucune musique actuellement.").await?;
    }
    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn skip(
    ctx: Context<'_>,
    #[description = "Nombre de musiques à passer"] count: Option<usize>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        let queue = handler.queue();
        let skip_count = count.unwrap_or(1);

        if queue.is_empty() {
            ctx.say("❌ Il n'y a rien à passer !").await?;
        } else {
            let current_len = queue.len();
            let to_skip = std::cmp::min(skip_count, current_len);
            
            for _ in 0..to_skip {
                let _ = queue.skip();
            }
            ctx.say(format!("⏭️ {} musique(s) passée(s) !", to_skip)).await?;
        }
    }
    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn shuffle(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();
    let manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();

    if let Some(handler_lock) = manager.get(guild_id) {
        let handler = handler_lock.lock().await;
        
        handler.queue().modify_queue(|q| {
            if q.len() > 2 {
                let mut vec: Vec<_> = q.drain(1..).collect();
                let mut rng = rand::rng(); 
                vec.shuffle(&mut rng);
                q.extend(vec);
            }
        });
        
        ctx.say("🔀 La file d'attente a été mélangée !").await?;
    }
    Ok(())
}

#[poise::command(slash_command, guild_only)]
async fn playlist(
    ctx: Context<'_>,
    #[description = "Lien de la playlist (YouTube ou Spotify)"] url: String,
) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_id = ctx.guild_id().unwrap();

    let channel_id = {
        let guild = ctx.guild().unwrap();
        guild.voice_states
            .get(&ctx.author().id)
            .and_then(|vs| vs.channel_id)
    };

    let manager = songbird::get(ctx.serenity_context())
        .await
        .expect("Songbird non initialisé")
        .clone();

    let handler_lock = if let Some(handler) = manager.get(guild_id) {
        handler
    } else {
        let connect_to = match channel_id {
            Some(c) => c,
            None => {
                ctx.say("❌ Tu dois être en vocal !").await?;
                return Ok(());
            }
        };
        manager.join(guild_id, connect_to).await?
    };

    // Récupération des URLs via yt-dlp (rapide car flat-playlist)
    let output = Command::new("yt-dlp")
        .args(["--flat-playlist", "--get-url", &url])
        .output()
        .await?;

    if !output.status.success() {
        ctx.say("❌ Erreur lors de la récupération de la playlist.")
            .await?;
        return Ok(());
    }

    let urls: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();

    if urls.is_empty() {
        ctx.say("❌ Playlist vide ou invalide.").await?;
        return Ok(());
    }

    let http_client = HttpClient::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
        .build()?;

    let total = urls.len();

    // ⚡ Traitement parallèle limité (10 en même temps)
    stream::iter(urls)
        .for_each_concurrent(10, |url| {
            let handler_lock = handler_lock.clone();
            let http = http_client.clone();

            async move {
                let src = YoutubeDl::new(http, url);

                let mut handler = handler_lock.lock().await;
                handler.enqueue_input(src.into()).await;
            }
        })
        .await;

    ctx.say(format!(
        "✅ Playlist chargée : {} musiques ajoutées rapidement 🚀",
        total
    ))
    .await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN manquant");

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![play(), queue(), skip(), shuffle(), playlist()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::GUILD_VOICE_STATES;
    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Erreur client");

    client.start().await.unwrap();
}