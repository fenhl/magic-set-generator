use {
    std::fmt,
    reqwest::StatusCode,
    serde_derive::Deserialize,
    //serde_json::json,
    //url::Url
};

#[derive(Deserialize)]
pub(crate) struct Branch {
    pub(crate) commit: Commit
}

#[derive(Deserialize)]
pub(crate) struct Release {
    //pub(crate) assets: Vec<ReleaseAsset>,
    //pub(crate) body: String,
    //pub(crate) id: u64,
    //pub(crate) name: String,
    pub(crate) tag_name: String,
    //pub(crate) upload_url: String
}

#[derive(Deserialize)]
pub(crate) struct ReleaseAsset {
    //pub(crate) name: String,
    //pub(crate) browser_download_url: Url
}

#[derive(Deserialize)]
pub(crate) struct Tag {
    pub(crate) name: String,
    pub(crate) commit: Commit
}

#[derive(Deserialize)]
pub(crate) struct Commit {
    pub(crate) sha: String
}

/// A GitHub repository. Provides API methods.
pub(crate) struct Repo {
    /// The GitHub user or organization who owns this repo.
    pub(crate) user: String,
    /// The name of the repo.
    pub(crate) name: String
}

impl Repo {
    pub(crate) fn new(user: impl ToString, name: impl ToString) -> Repo {
        Repo {
            user: user.to_string(),
            name: name.to_string()
        }
    }

    pub(crate) fn branch(&self, client: &reqwest::Client, name: impl fmt::Display) -> Result<Branch, reqwest::Error> {
        Ok(
            client.get(&format!("https://api.github.com/repos/{}/{}/branches/{}", self.user, self.name, name))
                .send()?
                .error_for_status()?
                .json()?
        )
    }

    pub(crate) fn latest_release(&self, client: &reqwest::Client) -> Result<Option<Release>, reqwest::Error> {
        let response = client.get(&format!("https://api.github.com/repos/{}/{}/releases/latest", self.user, self.name))
            .send()?;
        if response.status() == StatusCode::NOT_FOUND { return Ok(None); } // no releases yet
        Ok(Some(
            response.error_for_status()?
                .json::<Release>()?
        ))
    }

    /*
    /// Creates a draft release, which can be published using `Repo::publish_release`.
    pub(crate) fn create_release(&self, client: &reqwest::Client, name: String, tag_name: String, body: String) -> Result<Release, reqwest::Error> {
        Ok(
            client.post(&format!("https://api.github.com/repos/{}/{}/releases", self.user, self.name))
                .json(&json!({
                    "body": body,
                    "draft": true,
                    "name": name,
                    "tag_name": tag_name
                }))
                .send()?
                .error_for_status()?
                .json::<Release>()?
        )
    }

    pub(crate) fn publish_release(&self, client: &reqwest::Client, release: Release) -> Result<Release, reqwest::Error> {
        Ok(
            client.patch(&format!("https://api.github.com/repos/{}/{}/releases/{}", self.user, self.name, release.id))
                .json(&json!({"draft": false}))
                .send()?
                .error_for_status()?
                .json::<Release>()?
        )
    }

    pub(crate) fn release_attach(&self, client: &reqwest::Client, release: &Release, name: &str, content_type: &'static str, body: impl Into<reqwest::Body>) -> Result<ReleaseAsset, reqwest::Error> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::CONTENT_TYPE, reqwest::header::HeaderValue::from_static(content_type));
        Ok(
            client.post(&release.upload_url.replace("{?name,label}", ""))
                .query(&[("name", name)])
                .headers(headers)
                .body(body)
                .send()?
                .error_for_status()?
                .json::<ReleaseAsset>()?
        )
    }
    */

    pub(crate) fn tags(&self, client: &reqwest::Client) -> Result<Vec<Tag>, reqwest::Error> {
        Ok(
            client.get(&format!("https://api.github.com/repos/{}/{}/tags", self.user, self.name))
                .send()?
                .error_for_status()?
                .json::<Vec<Tag>>()?
        )
    }
}
