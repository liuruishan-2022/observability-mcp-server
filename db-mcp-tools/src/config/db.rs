///
/// 放置数据配置的配置信息
/// 主要包含两块：
/// 1. 数据库的基本信息
/// 2. MCP Tools动态工具的配置信息
///
use url::Url;

pub struct Database {
    username: String,
    password: String,
    host: String,
    port: u16,
    database: String,
}

impl Database {
    const USERNAME: &str = "USERNAME";
    const PASSWORD: &str = "PASSWORD";
    const HOST: &str = "HOST";
    const PORT: &str = "PORT";
    const DATABASE: &str = "DATABASE";

    //
    // 从环境变量中加载，毕竟这些信息都属于敏感信息,不能通过配置
    // 我们在k8s环境的情况下,通过secret加载到env中进行读取
    //
    pub fn load_from_env() -> Self {
        Self {
            username: std::env::var(Self::USERNAME).expect("环境变量:USERNAME不存在!"),
            password: std::env::var(Self::PASSWORD).expect("环境变量:PASSWORD不存在!"),
            host: std::env::var(Self::HOST).expect("环境变量:HOST不存在!"),
            port: std::env::var(Self::PORT)
                .expect("环境变量:PORT不存在!")
                .parse()
                .expect("PORT必须是数字!"),
            database: std::env::var(Self::DATABASE).expect("环境变量:DATABASE不存在!"),
        }
    }

    pub fn url(&self) -> String {
        let uri = format!("mysql://{}:{}", self.host, self.port);
        let mut uri = Url::parse(&uri).unwrap();
        let _ = uri.set_username(self.username());
        let _ = uri.set_password(Some(self.password()));

        return uri.as_str().to_string();
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn database(&self) -> &str {
        &self.database
    }
}
