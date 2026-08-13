#ifndef URL_VAULT_FFI_H
#define URL_VAULT_FFI_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Every operation returns one owned UTF-8 JSON envelope:
 * {"ok":true,"data":...} or {"ok":false,"error":"..."}.
 * Release it exactly once with url_vault_free_string.
 * A NULL home selects CODEX_URL_VAULT_HOME or the native default.
 */
void url_vault_free_string(char *value);

char *url_vault_init(const char *home);
char *url_vault_list_urls(const char *home, const char *category, uint32_t limit);
char *url_vault_search_urls(const char *home, const char *query, uint32_t limit);
char *url_vault_get_url(const char *home, const char *target);
char *url_vault_list_categories(const char *home);

char *url_vault_save_url(const char *home, const char *input_json);
char *url_vault_update_url(
    const char *home,
    const char *target,
    const char *changes_json
);
char *url_vault_move_url(
    const char *home,
    const char *target,
    const char *category
);
char *url_vault_archive_url(const char *home, const char *target);
char *url_vault_delete_url(const char *home, const char *target);

char *url_vault_create_category(
    const char *home,
    const char *path,
    const char *label,
    const char *note
);
char *url_vault_rename_category(
    const char *home,
    const char *old_path,
    const char *new_path
);
char *url_vault_archive_category(
    const char *home,
    const char *path,
    const char *move_to
);

char *url_vault_preview_import(
    const char *home,
    const char *html,
    const char *file_name,
    bool strip_common_root
);
char *url_vault_apply_import(const char *home, const char *input_json);

char *url_vault_save_snapshot(const char *home, const char *input_json);
char *url_vault_get_snapshot(const char *home, const char *target);
char *url_vault_verify_snapshots(const char *home, const char *target);
char *url_vault_open_url(
    const char *home,
    const char *target,
    const char *browser,
    bool dry_run
);

#ifdef __cplusplus
}
#endif

#endif
