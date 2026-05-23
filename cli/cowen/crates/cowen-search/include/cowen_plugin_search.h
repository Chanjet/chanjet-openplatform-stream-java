#ifndef COWEN_PLUGIN_SEARCH_H
#define COWEN_PLUGIN_SEARCH_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// Cowen Plugin Base ABI (Required for ALL plugins)
// ============================================================================

/**
 * @brief Get the standard name of the plugin.
 * @return A null-terminated C string representing the plugin name.
 */
const char* v1_name(void);

/**
 * @brief Get the description of the plugin.
 * @return A null-terminated C string describing what the plugin does.
 */
const char* v1_desc(void);

/**
 * @brief Get the supported trait/role of the plugin.
 * For search plugins, this MUST return exactly "SearchProvider".
 * @return A null-terminated C string representing the implemented trait.
 */
const char* v1_trait(void);

// ============================================================================
// Cowen SearchProvider Domain ABI (Required for SearchProvider plugins)
// ============================================================================

/**
 * @brief Receive and index API documents for searching.
 * 
 * @param docs_json A null-terminated C string containing a JSON Array of SearchDocument objects.
 * Format:
 * [
 *   {
 *     "id": "GET /api/v1/users",
 *     "summary": "Get users list",
 *     "description": "Returns a list of all users...",
 *     "vector": [] // usually empty when passed in, unless pre-computed
 *   }
 * ]
 * 
 * @return int32_t Returns 0 on success, non-zero on error.
 */
int32_t v1_index(const char* docs_json);

/**
 * @brief Perform a search against the indexed API documents.
 * 
 * @param query_ptr A null-terminated C string containing the user's search query.
 * @param top The maximum number of results to return.
 * 
 * @return const char* A null-terminated C string containing a JSON Array of result tuples.
 * Format:
 * [
 *   [ 0.95, { "id": "GET /api/v1/users", "summary": "Get users list", "description": "...", "vector": [] } ],
 *   [ 0.82, { "id": "POST /api/v1/users", "summary": "Create user", "description": "...", "vector": [] } ]
 * ]
 * Note: The memory of this returned string MUST remain valid until the next FFI call or until freed.
 * Implementing a static thread-local buffer is recommended.
 */
const char* v1_search(const char* query_ptr, size_t top);

/**
 * @brief Free resources and gracefully shutdown the plugin.
 * Called when Cowen is unloading the plugin or shutting down.
 */
void v1_free(void);

#ifdef __cplusplus
}
#endif

#endif // COWEN_PLUGIN_SEARCH_H
