use futures::future;
use reqwest::header::CONTENT_TYPE;
use rss::{
    Channel, ChannelBuilder, EnclosureBuilder, GuidBuilder, ImageBuilder, Item, ItemBuilder,
};
use std::path::Path;
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::trace;
use tracing::{error, info, info_span, Instrument};

pub mod config;
mod mangacross;

use crate::config::Config;
use crate::mangacross::{Comic, Episode, MangaCrossComic, MANGACROSS_HOST};

pub async fn build_rss(config: &Config, output: &Path) -> anyhow::Result<()> {
    info!("targets: {:?}", config.targets);

    let results = future::join_all(config.targets.iter().map(|target| {
        let span = info_span!("target", comic = target);
        async move {
            info!("Get {target}.json");
            let url = format!("{MANGACROSS_HOST}/api/comics/{target}.json?type=public");
            let res = reqwest::get(url).await?;
            let body = res.text().await?;

            let comic: MangaCrossComic = serde_json::from_str(body.as_str())?;

            info!("Create feed start");
            let channel = comic_to_channel(&comic.comic).await?;
            let feed = channel.to_string();

            let dir = &Path::new(output).join(target);
            info!("Create {:?} dir", dir);
            fs::create_dir_all(dir).await?;

            let file = &dir.join("feed.xml");
            info!("Write to {:?}", file);
            let mut file = File::create(file).await?;
            file.write_all(feed.as_bytes()).await?;
            Ok::<(), anyhow::Error>(())
        }
        .instrument(span)
    }))
    .await;

    for ref result in &results {
        if let Err(e) = result {
            error!("{:?}", e)
        }
    }

    if results.iter().all(|r| r.is_ok()) {
        Ok(())
    } else {
        anyhow::bail!("Fail build RSS")
    }
}

#[tracing::instrument(skip_all, fields(comic = comic.dir_name))]
pub async fn comic_to_channel(comic: &Comic) -> anyhow::Result<Channel> {
    trace!("to_channel {} start", comic.title);
    let mut channel = ChannelBuilder::default()
        .title(comic.title.clone())
        .link(format!(
            "{}/comics/{}/",
            MANGACROSS_HOST,
            comic.dir_name.clone()
        ))
        .description(comic.caption_for_search.clone())
        .image(
            ImageBuilder::default()
                .url(comic.image_url.clone())
                .link(comic.image_url.clone())
                .title(format!("{} {}", &comic.title, &comic.author))
                .build(),
        )
        .pub_date(comic.latest_episode_publish_start.clone())
        .last_build_date(comic.latest_episode_publish_start.clone())
        .build();

    let items: Vec<Item> = future::try_join_all(
        comic
            .episodes
            .iter()
            .filter(|ep| ep.status == "public")
            .map(|ep| episode_to_item(ep, comic)),
    )
    .await?;
    channel.set_items(items);

    trace!("to_channel {} done", comic.title);
    Ok(channel)
}

#[tracing::instrument(skip_all, fields(volume = episode.sort_volume))]
pub async fn episode_to_item(episode: &Episode, comic: &Comic) -> anyhow::Result<Item> {
    trace!("episode_to_item {} start", episode.sort_volume);
    let mut item = ItemBuilder::default();
    let guid = GuidBuilder::default()
        .value(format!("{}{}", MANGACROSS_HOST, episode.page_url))
        .permalink(true)
        .build();
    trace!(
        "episode_to_item {} download image start",
        episode.sort_volume
    );
    let image_res = reqwest::get(episode.list_image_double_url.as_str()).await?;
    trace!(
        "episode_to_item {} download image done",
        episode.sort_volume
    );
    let mime_type = match image_res.headers().get(CONTENT_TYPE) {
        Some(content_type) => content_type.to_str()?,
        None => "",
    };
    let length = match image_res.content_length() {
        Some(n) => n.to_string(),
        None => "".to_string(),
    };
    let enclosure = EnclosureBuilder::default()
        .url(episode.list_image_double_url.clone())
        .mime_type(mime_type)
        .length(length)
        .build();
    trace!("episode_to_item {} done", episode.sort_volume);

    Ok(item
        .title(format!("{} | {}", episode.volume, episode.title))
        .link(format!("{}{}", MANGACROSS_HOST, episode.page_url))
        .guid(guid)
        .pub_date(episode.publish_start.clone())
        .author(comic.author.clone())
        .enclosure(enclosure)
        .build())
}
