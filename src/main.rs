use poise::serenity_prelude as serenity;
use dotenvy::dotenv;
use std::env;


struct Data {}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;


#[poise::command(slash_command)]
async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Prêt à gérer le serveur").await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn launch_serv(ctx: Context<'_>) -> Result<(), Error> {
    let ip_nas = env::var("NAS_IP").expect("NAS_IP manquant dans le .env");
    let mut response = String::new();
    if ip_nas != "0.0.0.0" {
        response.push_str(&format!("Connexion au NAS avec l'IP : {}", ip_nas));
        
    }
    response.push_str("\nLancement du serveur (manque encore le lien avec le nas)");
    ctx.say(response).await?;
    Ok(())
}

#[poise::command(slash_command)]
async fn playlist_add(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Ajout de la playlist").await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv().expect("Fichier .env introuvable");
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN manquant dans le .env");
    
    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder().options(poise::FrameworkOptions {
            commands: vec![ping(), launch_serv(), playlist_add()], 
            ..Default::default()
        }).setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                println!("Commandes Slash enregistrées sur Discord !");
                Ok(Data {})
            })
        })
        .build();


    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await
        .expect("Erreur lors de la création du client Discord");

    println!("Connexion à Discord en cours...");
    if let Err(why) = client.start().await {
        println!("Erreur fatale : {:?}", why);
    }

}
