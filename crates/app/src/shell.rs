use std::fs;
use std::io;
use std::path::Path;

const ZSHENV: &str = r#"# yatty: keep the user's history file in $HOME, not in this directory
[[ "$HISTFILE" == "$ZDOTDIR/.zsh_history" ]] && HISTFILE="$HOME/.zsh_history"
[[ -f "$HOME/.zshenv" ]] && source "$HOME/.zshenv"
"#;

const ZPROFILE: &str = r#"[[ -f "$HOME/.zprofile" ]] && source "$HOME/.zprofile"
"#;

const ZLOGIN: &str = r#"[[ -f "$HOME/.zlogin" ]] && source "$HOME/.zlogin"
"#;

const ZSHRC: &str = r#"[[ "$HISTFILE" == "$ZDOTDIR/.zsh_history" ]] && HISTFILE="$HOME/.zsh_history"
[[ -f "$HOME/.zshrc" ]] && source "$HOME/.zshrc"

# yatty prompt, in the oh-my-zsh "robbyrussell" style
autoload -Uz colors && colors
setopt PROMPT_SUBST

_yatty_git_info() {
  local ref dirty
  ref=$(GIT_OPTIONAL_LOCKS=0 command git symbolic-ref --quiet --short HEAD 2>/dev/null) \
    || ref=$(GIT_OPTIONAL_LOCKS=0 command git rev-parse --short HEAD 2>/dev/null) || return 0
  if [[ -n $(GIT_OPTIONAL_LOCKS=0 command git status --porcelain 2>/dev/null | head -n 1) ]]; then
    dirty=" %{$fg[yellow]%}✗"
  fi
  print -rn -- "%{$fg_bold[blue]%}git:(%{$fg[red]%}${ref//\%/%%}%{$fg[blue]%})${dirty}%{$reset_color%} "
}

PROMPT='%(?:%{$fg_bold[green]%}➜ :%{$fg_bold[red]%}➜ ) %{$fg[cyan]%}%c%{$reset_color%} $(_yatty_git_info)'
"#;

/// Writes the zsh startup files that add the yatty prompt and returns the environment
/// variables to start zsh with. Returns an empty list for other shells.
pub fn zsh_env(config_dir: &Path, shell: &str) -> io::Result<Vec<(String, String)>> {
    if !shell.ends_with("zsh") {
        return Ok(Vec::new());
    }
    let dir = config_dir.join("shell").join("zsh");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(".zshenv"), ZSHENV)?;
    fs::write(dir.join(".zprofile"), ZPROFILE)?;
    fs::write(dir.join(".zlogin"), ZLOGIN)?;
    fs::write(dir.join(".zshrc"), ZSHRC)?;
    Ok(vec![("ZDOTDIR".into(), dir.to_string_lossy().into_owned())])
}
