use super::*;
use rand::RngExt;
use serenity::async_trait;

#[derive(Debug)]
pub struct FemboyCommand;

static FEMBOY_IMAGES: [&str; 4] = [
    "https://i.pinimg.com/originals/10/89/22/1089220beba27d80631e65408f045df8.gif",
    "https://i.pinimg.com/originals/4b/48/5b/4b485b307c58b4a77a19f755d1620388.gif",
    "https://i.pinimg.com/originals/c8/38/5c/c8385c6980bcac8c8a370f815d48ece0.gif",
    "https://i.pinimg.com/736x/cf/2d/45/cf2d456b0f88017dbbbc6f8c9d9f7d17.jpg",
];

#[async_trait]
impl DiscordCommandHandler for FemboyCommand {
    fn definition(&self) -> DiscordCommandDefinition {
        DiscordCommandDefinition::new_global("femboy", true, false)
    }

    fn registration(&self) -> CreateCommand {
        CreateCommand::new("femboy")
            .name_localized("ru", "фембой")
            .description(">w<")
    }

    async fn handler(&self, _opts: &[ResolvedOption]) -> DiscordCommandResponse {
        let mut rng = rand::rng();
        let random_index = rng.random_range(0..FEMBOY_IMAGES.len());
        let random_img = FEMBOY_IMAGES[random_index];

        DiscordCommandResponse::followup_response(random_img, false)
    }
}
