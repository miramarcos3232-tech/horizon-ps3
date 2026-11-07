#include <stddef.h>
#include <stdint.h>

#include <lv2/sysfs.h>
#include <net/netdb.h>
#include <net/socket.h>
#include <sys/thread.h>

_Static_assert(sizeof(void *) == 8, "PS3 PPU pointers must be 64-bit");
_Static_assert(sizeof(size_t) == 8, "PS3 size_t must be 64-bit");
_Static_assert(sizeof(sys_ppu_thread_t) == 8, "PSL1GHT thread handles must be 64-bit");
_Static_assert(sizeof(socklen_t) == 4, "PSL1GHT socklen_t must be u32");
_Static_assert(sizeof(sa_family_t) == 1, "PSL1GHT sa_family_t must be u8");
_Static_assert(offsetof(struct sockaddr, sa_data) == 2, "PSL1GHT sockaddr header layout changed");
_Static_assert(sizeof(struct iovec) == 16, "PSL1GHT iovec layout changed");
_Static_assert(sizeof(sysFSStat) == 52, "PSL1GHT sysFSStat packed layout changed");
_Static_assert(offsetof(sysFSStat, st_size) == 36, "PSL1GHT sysFSStat size offset changed");
_Static_assert(sizeof(struct addrinfo) == 48, "PSL1GHT addrinfo layout changed");

/* These references intentionally preserve the SDK's real contracts. */
__attribute__((used)) static s32 (*const probe_fs_stat)(const char *, sysFSStat *) = sysFsStat;
__attribute__((used)) static int (*const probe_socket)(int, int, int) = socket;
__attribute__((used)) static struct hostent *(*const probe_host_by_name)(const char *) = gethostbyname;
__attribute__((used)) static s32 (*const probe_thread_create)(sys_ppu_thread_t *, void (*)(void *), void *, s32,
                                                               u64, u64, char *) = sysThreadCreate;

int horizon_ps3_abi_contract_probe(void) {
    return 4;
}
