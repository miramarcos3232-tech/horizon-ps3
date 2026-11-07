#include "psl1ght_bridge.h"

/* Entry for the experimental flying-camera world client. */
int main(void) {
    if (horizon_ps3_rust_abi_version() != 1u) return 1;
    return horizon_ps3_rust_world_entry();
}
