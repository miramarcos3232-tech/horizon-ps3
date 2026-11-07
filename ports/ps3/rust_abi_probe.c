#include "psl1ght_bridge.h"

int main(void) {
    if (horizon_ps3_rust_abi_version() != 1u) return 1;
    return horizon_ps3_rust_smoke_entry();
}
