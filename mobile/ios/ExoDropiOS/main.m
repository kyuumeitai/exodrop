// Thin native entry point handing control directly to the Rust/winit/wgpu engine.
extern void exodrop_ios_main(void);

int main(int argc, char *argv[]) {
    (void)argc;
    (void)argv;
    exodrop_ios_main();
    return 0;
}
