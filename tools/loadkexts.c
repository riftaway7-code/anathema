typedef unsigned long u64;

#define LOAD_KEXT 0xffffff80009850f0ul
#define KPRINTF 0xffffff8000299590ul
#define MAC_POLICY_REGISTER 0xffffff8000ab9a00ul
#define SBXBASE 0xaabbccddeeff1122ul

typedef int (*load_t)(const char *, int, int, unsigned short, unsigned short, void *);
typedef int (*reg_t)(void *, void *, void *);

static const char k1[] __attribute__((section(".text"))) = "com.apple.filesystems.tmpfs";
static const char k2[] __attribute__((section(".text"))) = "com.apple.AppleFSCompression.AppleFSCompressionTypeZlib";
static const char k3[] __attribute__((section(".text"))) = "com.apple.AppleFSCompression.AppleFSCompressionTypeDataless";
static const char fmt[] __attribute__((section(".text"))) = "anx loadkext %s: %d\n";
static const char fmt2[] __attribute__((section(".text"))) = "anx sandbox stand-in registered: %d\n";
static const char nm[] __attribute__((section(".text"))) = "Sandbox";
static const char fn[] __attribute__((section(".text"))) = "Sandbox stand-in";

static int sbx_syscall(void *p, int call, void *arg)
{
	(void)p;
	(void)call;
	(void)arg;
	return 0;
}

static void register_sandbox(void)
{
	unsigned char *conf = (unsigned char *)SBXBASE;
	unsigned char *ops = conf + 0x60;
	unsigned char *handle = conf + 0x50;
	*(const char **)(conf + 0x0) = nm;
	*(const char **)(conf + 0x8) = fn;
	*(void **)(conf + 0x20) = ops;
	*(void **)(ops + 0x3a8) = (void *)sbx_syscall;
	int r = ((reg_t)MAC_POLICY_REGISTER)(conf, handle, 0);
	((int (*)(const char *, ...))KPRINTF)(fmt2, r);
}

static void one(const char *id)
{
	int r = ((load_t)LOAD_KEXT)(id, 0, 0, 0, 2, 0);
	((int (*)(const char *, ...))KPRINTF)(fmt, id, r);
}

void anx_loadkexts(void)
{
	register_sandbox();
	one(k1);
	one(k2);
	one(k3);
}

__asm__(
	".section .anxcave,\"ax\"\n"
	".globl anx_cave3\n"
	"anx_cave3:\n"
	"push %rdi\n push %rsi\n push %rdx\n push %rcx\n push %r8\n push %r9\n"
	"sub $8,%rsp\n"
	"call anx_loadkexts\n"
	"add $8,%rsp\n"
	"pop %r9\n pop %r8\n pop %rcx\n pop %rdx\n pop %rsi\n pop %rdi\n"
	"push %rbp\n mov %rsp,%rbp\n push %r15\n"
	"jmp *1f(%rip)\n"
	"1: .quad 0xffffff80007e0bc6\n"
	".text\n");
