/* The old-app fixture (DX7.11): a C app built against a released 2.N header, run by `cargo run -p gate -- abi-compat` against the current
   library. It states the version it was built for, writes the records at the sizes that release had and calls only what that release
   exported. Rebuilt for a release with tools/abi-fixture.sh; the binary is then kept in fixtures/abi/2.N/. */
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
    if ((version >> 16) != 2 || (version & 0xffff) < PXUI_FIXTURE_MINOR) FAIL("the library is older than the release this app was built against");
    if (pxui_renderer_composite(NULL, NULL, NULL, NULL) != PXUI_ERR_ARGUMENT) FAIL("pxui_renderer_composite");
    PxuiUiDesc desc = { sizeof desc, 0, 13, PXUI_ABI_BUILT };
    int32_t status = 0;
    PxuiUi *ui = pxui_ui_create(&desc, NULL, 0, &status);
    if (!ui) FAIL("pxui_ui_create");
    if (pxui_ui_add_surface(ui, 1) != 0) FAIL("pxui_ui_add_surface");
    for (int frame = 0; frame < 3; frame++) {
        PxuiFrameIn in;
        memset(&in, 0, sizeof in);
        in.size = (uint32_t)sizeof in;
        in.surface = 1;
        in.events.stride = sizeof(PxuiEvent);
        in.environment.size = sizeof in.environment;
        in.keyboard.size = sizeof in.keyboard;
        in.width = 320.0f; in.height = 240.0f; in.scale = 1.0f; in.time = frame / 60.0;
        PxuiFrameOut out;
        memset(&out, 0, sizeof out);
        out.size = sizeof out;
        if (pxui_frame(ui, &in, build, NULL, &out) != 0) FAIL("pxui_frame refused the release's input");
        const PxuiOutput *output = pxui_ui_output(ui);
        if (!output || output->size < sizeof *output) FAIL("pxui_ui_output omitted the release's fields");
        PxuiDamageList damage = {0};
        if (pxui_ui_damage_list(ui, &damage) != 0 ||
            damage.count != output->damage_list.count || damage.full != output->damage_list.full) {
            FAIL("the damage list disagrees with the release's output layout");
        }
    }
    const PxuiPromoted *promoted = NULL;
    size_t count = 1;
    if (pxui_ui_promoted(ui, &promoted, &count) != 0 || count != 0) FAIL("pxui_ui_promoted");
    if (clicked_calls == 0) FAIL("the button call never succeeded");
    if (pxui_ui_destroy(ui) != 0) FAIL("pxui_ui_destroy");
    printf("old app ok: built for 2.%d, running on %u.%u\n", PXUI_FIXTURE_MINOR, version >> 16, version & 0xffff);
    return 0;
}
