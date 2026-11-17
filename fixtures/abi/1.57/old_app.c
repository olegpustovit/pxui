/* The old-app fixture (DX7.11): a C app built against an older release's header, run by `cargo run -p gate -- abi-compat` against the
   current library. It writes the records at the sizes that release had (frame inputs of that release's size) and calls only what
   that release exported. Rebuilt for a release with tools/abi-fixture.sh; the binary is then kept in fixtures/abi/<major.minor>/. */
#include <pxui.h>
#include <stdio.h>
#include <string.h>

#define FAIL(what) do { fprintf(stderr, "old app: %s\n", what); return 1; } while (0)

static int clicked_calls = 0;

static void build(void *user, PxuiUi *ui) {
    PxuiResponse response;
    memset(&response, 0, sizeof response);
    response.size = sizeof response;
    (void)user;
    PxuiStr key = { (const uint8_t *)"ok", 2 }, text = { (const uint8_t *)"OK", 2 };
    if (pxui_ui_button_plain(ui, key, text, &response) == 0) clicked_calls++;
}

int main(void) {
    uint32_t version = pxui_abi_version();
    if ((version >> 16) != 1 || (version & 0xffff) < PXUI_FIXTURE_MINOR) FAIL("the library is older than the release this app was built against");
    PxuiUiDesc desc = { sizeof desc, 0, 13 };
    PxuiUi *ui = pxui_ui_create(&desc);
    if (!ui) FAIL("pxui_ui_create");
    /* the frame input at the size that release defined (and the oldest, 48-byte prefix): later libraries read both */
    size_t sizes[2] = { offsetof(PxuiFrameIn, appearance), sizeof(PxuiFrameIn) };
    for (int s = 0; s < 2; s++) {
        for (int frame = 0; frame < 3; frame++) {
            PxuiFrameIn in;
            memset(&in, 0, sizeof in);
            in.size = (uint32_t)sizes[s];
            in.width = 320.0f; in.height = 240.0f; in.scale = 1.0f; in.time = frame / 60.0;
            PxuiFrameOut out;
            memset(&out, 0, sizeof out);
            out.size = sizeof out;
            if (pxui_frame(ui, &in, build, NULL, &out) != 0) FAIL("pxui_frame refused an old-size input");
        }
    }
    if (clicked_calls == 0) FAIL("the button call never succeeded");
    if (pxui_ui_destroy(ui) != 0) FAIL("pxui_ui_destroy");
    printf("old app ok: built for 1.%d, running on %u.%u\n", PXUI_FIXTURE_MINOR, version >> 16, version & 0xffff);
    return 0;
}
