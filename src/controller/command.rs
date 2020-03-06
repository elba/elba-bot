use crate::config::CONFIG;
use crate::error::Result;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Publish {
        git: String,
        refname: Option<String>,
    },
}

impl Command {
    pub fn from_str(s: &str) -> Result<Option<Self>> {
        let (_, command) =
            parse::parse_command(s, &CONFIG.bot_name).map_err(|err| err.to_owned())?;
        Ok(command)
    }
}

mod parse {
    use super::Command;
    use nom::{bytes::complete::*, character::complete::*, combinator::opt, IResult};

    pub fn parse_command<'a>(i: &'a str, bot_name: &'a str) -> IResult<&'a str, Option<Command>> {
        let (i, _) = multispace0(i)?;

        let (i, mention) = opt(mention(bot_name))(i)?;
        if mention.is_none() {
            return Ok((i, None));
        }

        let (i, _) = multispace1(i)?;
        let (i, command) = parse_publish(i)?;

        Ok((i, Some(command)))
    }

    fn mention<'a>(bot_name: &'a str) -> impl Fn(&'a str) -> IResult<&'a str, ()> {
        move |i: &str| {
            let (i, _) = tag("@")(i)?;
            let (i, _) = tag(bot_name)(i)?;
            Ok((i, ()))
        }
    }

    fn parse_publish(i: &str) -> IResult<&str, Command> {
        let (i, _) = tag("/publish")(i)?;
        let (i, _) = multispace1(i)?;

        let (i, git) = word(i)?;
        let (i, refname) = opt(|i| {
            let (i, _) = multispace1(i)?;
            word(i)
        })(i)?;

        Ok((
            i,
            Command::Publish {
                git: git.to_owned(),
                refname: refname.map(ToString::to_string),
            },
        ))
    }

    fn word(i: &str) -> IResult<&str, &str> {
        take_while1(|c: char| !c.is_whitespace())(i)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_command() {
        let cases = vec![
            ("@na", None),
            ("@", None),
            (
                "@name /publish abc.xyz/zz.git",
                Some(Command::Publish {
                    git: "abc.xyz/zz.git".to_owned(),
                    refname: None,
                }),
            ),
            (
                "@name  /publish abc",
                Some(Command::Publish {
                    git: "abc".to_owned(),
                    refname: None,
                }),
            ),
            (
                "@name /publish abc.xyz/zz.git master",
                Some(Command::Publish {
                    git: "abc.xyz/zz.git".to_owned(),
                    refname: Some("master".to_owned()),
                }),
            ),
            (
                "@name /publish abc.xyz/zz.git master more",
                Some(Command::Publish {
                    git: "abc.xyz/zz.git".to_owned(),
                    refname: Some("master".to_owned()),
                }),
            ),
        ];

        for (text, expected) in cases {
            assert_eq!(parse::parse_command(text, "name").unwrap().1, expected);
        }
    }

    #[test]
    fn test_parse_command_fail() {
        let cases = vec![
            "@name / publish abc",
            "@name /publis abc",
            "@name / abc",
            "@name/publish abc.xyz/zz.git",
        ];

        for text in cases {
            assert!(parse::parse_command(text, "name").is_err());
        }
    }
}
