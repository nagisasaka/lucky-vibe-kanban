/* Isolated switch fixture, never a deployment binary. */
#include <arpa/inet.h>
/* Only the system runtime library is required; no SDK installation. */
typedef struct sqlite3 sqlite3;
extern int sqlite3_open(const char *, sqlite3 **);
extern int sqlite3_exec(sqlite3 *, const char *, void *, void *, char **);
#define SQLITE_OK 0
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>
#ifndef REVISION
#define REVISION 1
#endif
#ifndef FRONTEND
#define FRONTEND "/missing/frontend"
#endif
#ifndef PAYLOAD
#define PAYLOAD "{}"
#endif
#ifndef HTML
#define HTML "fixture"
#endif
int main(int argc, char **argv) {
  if (argc == 2 && strcmp(argv[1], "runtime-payload") == 0) { const char *name = strrchr(argv[0], '/'); name = name ? name + 1 : argv[0];
    if (strcmp(name, "server") == 0) puts(PAYLOAD);
    else printf("{\"component\":\"%s\",\"protocol\":1}\n", name);
    return 0; }
  if (getenv("FIXTURE_CHILD") && fork() == 0) {
    /* No data FD exists yet. A real server child must still prevent cutover. */
    sleep(30);
    return 0;
  }
  char path[4096];
  snprintf(path, sizeof(path), "%s/data/db.v2.sqlite", getenv("LVK_INSTANCE_DIR"));
  sqlite3 *db = NULL;
  if (sqlite3_open(path, &db) != SQLITE_OK) return 2;
  /* Migration is performed by the fixture server at startup, never the engine. */
  if (REVISION == 2 && sqlite3_exec(db, "CREATE TABLE IF NOT EXISTS migrated(value TEXT); PRAGMA user_version=2;", NULL, NULL, NULL) != SQLITE_OK) return 3;
  if (getenv("FIXTURE_EXIT")) return 4;
  int sock = socket(AF_INET, SOCK_STREAM, 0), yes = 1;
  setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &yes, sizeof(yes));
  struct sockaddr_in addr = {.sin_family=AF_INET, .sin_addr.s_addr=htonl(INADDR_LOOPBACK), .sin_port=htons(atoi(getenv("BACKEND_PORT")))};
  if (bind(sock, (struct sockaddr*)&addr, sizeof(addr)) || listen(sock, 8)) return 5;
  while (1) {
    int client = accept(sock, NULL, NULL);
    if (client < 0) return 6;
    char request[8192]={0};
    read(client, request, sizeof(request)-1);
    const char *body = "{\"success\":true}";
    if (strstr(request, "/api/info")) body="{\"success\":true,\"data\":{\"build_info\":{\"kind\":\"fixture\"}}}";
    else if (strstr(request, "/api/resource-coordination/snapshot")) body="{\"success\":true,\"data\":{\"resources\":[{\"id\":\"unused-fixture-device\",\"description\":\"Unused peripheral unrelated to this instance\"}],\"holders\":[],\"queue\":[]}}";
    else if (strstr(request, "GET /index.html ")) {
      body=HTML;
    }
    else if (strstr(request, "GET / ")) body=FRONTEND;
    else if (getenv("FIXTURE_BAD_HEALTH")) body="{\"success\":false}";
    dprintf(client,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s",strlen(body),body);
    close(client);
  }
}
