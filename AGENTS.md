# Codex URL Vault Native Constitution

このファイルはこのrepository root配下における Codex の局所 `AGENTS.md` であり、このnative projectの正本、identity、実装、検証、公開条件を定義する SSOT である。
本書は助言集ではなく、source/runtime境界、project identity、portable distribution、検証、GitHub integrationを拘束する運用契約として扱う。
上位の `AGENTS.md`、システム指示、開発者指示、ユーザーの明示要求と競合する場合は、Codex の優先順位規則に従う。

## Project Identity And Source Boundary

- このrepositoryは`codex-url-vault-native`という独立projectである。内部plugin identityの`codex-url-vault`、互換data、類似機能を理由に、非native `codex-url-vault` projectのsource、commit、branch、tag、remote、release、issue、pull requestを混在させない。
- Rust、SwiftUI、Brave extension、Native Messaging Host、public plugin packageの正本はこのrepositoryとする。
- `~/.claude/plugins/cache/`、installed app、`~/.codex/url-vault`、SQLite、bookmark HTML、snapshot、socket、`target/`、`dist/`、`macos/.build/`はruntimeまたはgenerated artifactであり、一次編集先やGitHub公開sourceにしない。
- GitHub公開先は同一identityの`mlabo-org/codex-url-vault-native`だけとし、非native repositoryを上書き、rename、履歴統合して代用しない。

## Portable Runtime Contract

- shipped sourceに特定ユーザーのhome absolute pathを固定しない。既定app pathは`$HOME/Applications/Codex URL Vault.app`から解決し、`CODEX_URL_VAULT_APP_PATH`だけを明示overrideとして扱う。
- tracked Brave Native Messaging manifestはportable templateとする。実登録には`url-vault-native-host --print-brave-manifest <absolute-host-path>`が生成したmanifestを使い、template placeholderをそのまま登録しない。
- plugin MCP launcherはinstalled app内のnative binaryを`exec`する薄い起動境界だけを所有し、Vault処理、fallback実装、SQLite操作を持たない。

## Verification And Operational Boundaries

- Rust sourceを変更した場合は、変更surfaceを含む`cargo test --workspace`を実行する。
- plugin MCP launcherを変更した場合は、isolated temporary app pathにfake executableを置き、`CODEX_URL_VAULT_APP_PATH`でそのbinaryを起動できることをsmoke確認する。既定pathのportable性はsource内の特定ユーザーabsolute pathゼロ一致とRust path-resolution testで確認する。
- Brave extensionまたはhost manifest contractを変更した場合は`node scripts/validate-brave-extension.mjs`を実行する。
- plugin manifest、MCP config、marketplace metadataを変更した場合はplugin creatorのvalidatorとmarketplace validatorを同じacceptance bundleで実行する。
- SwiftUI sourceまたはapp packagingを変更した場合は`./scripts/build-release.sh`をrunnable app acceptanceとして使う。
- source completion、Git commit、GitHub push、plugin refresh、app installation、Brave extension loading、Native Messaging registration、runtime activationは別actionとする。現在の明示範囲に含まれない後段を推定実行しない。

## Maintenance Gates

- この`AGENTS.md`を作成、改訂、または実質変更した場合、final reportやcommitの前に`agents-md-clarifier`を適用する。
- plugin-contained `SKILL.md`を実質変更した場合、final reportやcommitの前に`skill-md-clarifier`を適用する。
- 編集前後に`git status --short --branch`を確認し、runtime data、secret、credential、既存の無関係なuser changeをcommitまたは公開しない。
