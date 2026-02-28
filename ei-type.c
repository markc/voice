/*
 * ei-type — type text into the focused window via KWin EIS + libei
 *
 * Connects to org.kde.KWin.EIS.RemoteDesktop on D-Bus, gets a libei fd,
 * negotiates a keyboard device, and injects evdev key events for each
 * character read from stdin.
 *
 * Build: gcc -O2 -o ei-type ei-type.c $(pkg-config --cflags --libs libei-1.0) -lsystemd
 * Usage: echo "Hello, World!" | ei-type
 */

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <unistd.h>
#include <signal.h>
#include <poll.h>
#include <ctype.h>
#include <errno.h>
#include <getopt.h>
#include <fcntl.h>

#include <libei.h>
#include <systemd/sd-bus.h>

static bool g_verbose = false;
#define DBG(...) do { if (g_verbose) fprintf(stderr, "ei-type: " __VA_ARGS__); } while(0)

/* Default inter-key delay in microseconds */
#define DEFAULT_DELAY_US 5000

/* Evdev keycodes (linux/input-event-codes.h values) */
#define KEY_ESC       1
#define KEY_1         2
#define KEY_2         3
#define KEY_3         4
#define KEY_4         5
#define KEY_5         6
#define KEY_6         7
#define KEY_7         8
#define KEY_8         9
#define KEY_9        10
#define KEY_0        11
#define KEY_MINUS    12
#define KEY_EQUAL    13
#define KEY_TAB      15
#define KEY_Q        16
#define KEY_W        17
#define KEY_E        18
#define KEY_R        19
#define KEY_T        20
#define KEY_Y        21
#define KEY_U        22
#define KEY_I        23
#define KEY_O        24
#define KEY_P        25
#define KEY_LEFTBRACE  26
#define KEY_RIGHTBRACE 27
#define KEY_ENTER    28
#define KEY_A        30
#define KEY_S        31
#define KEY_D        32
#define KEY_F        33
#define KEY_G        34
#define KEY_H        35
#define KEY_J        36
#define KEY_K        37
#define KEY_L        38
#define KEY_SEMICOLON 39
#define KEY_APOSTROPHE 40
#define KEY_GRAVE    41
#define KEY_LEFTSHIFT 42
#define KEY_BACKSLASH 43
#define KEY_Z        44
#define KEY_X        45
#define KEY_C        46
#define KEY_V        47
#define KEY_B        48
#define KEY_N        49
#define KEY_M        50
#define KEY_COMMA    51
#define KEY_DOT      52
#define KEY_SLASH    53
#define KEY_SPACE    57
#define KEY_LEFTCTRL  29
#define KEY_LEFTALT   56
#define KEY_LEFTMETA 125

/* libei device capabilities (bitmask, matches enum ei_device_capability) */
#define CAP_POINTER          (1 << 0)
#define CAP_POINTER_ABSOLUTE (1 << 1)
#define CAP_KEYBOARD         (1 << 2)
#define CAP_TOUCH            (1 << 3)
#define CAP_SCROLL           (1 << 4)
#define CAP_BUTTON           (1 << 5)
#define CAP_ALL (CAP_POINTER | CAP_POINTER_ABSOLUTE | CAP_KEYBOARD | CAP_TOUCH | CAP_SCROLL | CAP_BUTTON)

static volatile sig_atomic_t g_quit = 0;

static void sighandler(int sig) {
    (void)sig;
    g_quit = 1;
}

struct keyinfo {
    uint32_t code;
    bool     shift;
};

static struct keyinfo char_to_key(char c) {
    struct keyinfo k = {0, false};

    /* lowercase letters */
    if (c >= 'a' && c <= 'z') {
        static const uint32_t az[] = {
            KEY_A, KEY_B, KEY_C, KEY_D, KEY_E, KEY_F, KEY_G, KEY_H, KEY_I,
            KEY_J, KEY_K, KEY_L, KEY_M, KEY_N, KEY_O, KEY_P, KEY_Q, KEY_R,
            KEY_S, KEY_T, KEY_U, KEY_V, KEY_W, KEY_X, KEY_Y, KEY_Z
        };
        k.code = az[c - 'a'];
        return k;
    }

    /* uppercase letters */
    if (c >= 'A' && c <= 'Z') {
        static const uint32_t az[] = {
            KEY_A, KEY_B, KEY_C, KEY_D, KEY_E, KEY_F, KEY_G, KEY_H, KEY_I,
            KEY_J, KEY_K, KEY_L, KEY_M, KEY_N, KEY_O, KEY_P, KEY_Q, KEY_R,
            KEY_S, KEY_T, KEY_U, KEY_V, KEY_W, KEY_X, KEY_Y, KEY_Z
        };
        k.code = az[c - 'A'];
        k.shift = true;
        return k;
    }

    /* digits */
    if (c >= '1' && c <= '9') {
        k.code = KEY_1 + (c - '1');
        return k;
    }
    if (c == '0') { k.code = KEY_0; return k; }

    /* space, enter, tab */
    if (c == ' ')  { k.code = KEY_SPACE; return k; }
    if (c == '\n') { k.code = KEY_ENTER; return k; }
    if (c == '\t') { k.code = KEY_TAB;   return k; }

    /* punctuation (unshifted) */
    switch (c) {
        case '-':  k.code = KEY_MINUS;      return k;
        case '=':  k.code = KEY_EQUAL;      return k;
        case '[':  k.code = KEY_LEFTBRACE;  return k;
        case ']':  k.code = KEY_RIGHTBRACE; return k;
        case '\\': k.code = KEY_BACKSLASH;  return k;
        case ';':  k.code = KEY_SEMICOLON;  return k;
        case '\'': k.code = KEY_APOSTROPHE; return k;
        case '`':  k.code = KEY_GRAVE;      return k;
        case ',':  k.code = KEY_COMMA;      return k;
        case '.':  k.code = KEY_DOT;        return k;
        case '/':  k.code = KEY_SLASH;      return k;
    }

    /* shifted punctuation */
    k.shift = true;
    switch (c) {
        case '!': k.code = KEY_1;          return k;
        case '@': k.code = KEY_2;          return k;
        case '#': k.code = KEY_3;          return k;
        case '$': k.code = KEY_4;          return k;
        case '%': k.code = KEY_5;          return k;
        case '^': k.code = KEY_6;          return k;
        case '&': k.code = KEY_7;          return k;
        case '*': k.code = KEY_8;          return k;
        case '(': k.code = KEY_9;          return k;
        case ')': k.code = KEY_0;          return k;
        case '_': k.code = KEY_MINUS;      return k;
        case '+': k.code = KEY_EQUAL;      return k;
        case '{': k.code = KEY_LEFTBRACE;  return k;
        case '}': k.code = KEY_RIGHTBRACE; return k;
        case '|': k.code = KEY_BACKSLASH;  return k;
        case ':': k.code = KEY_SEMICOLON;  return k;
        case '"': k.code = KEY_APOSTROPHE; return k;
        case '~': k.code = KEY_GRAVE;      return k;
        case '<': k.code = KEY_COMMA;      return k;
        case '>': k.code = KEY_DOT;        return k;
        case '?': k.code = KEY_SLASH;      return k;
    }

    /* unmapped character */
    k.code = 0;
    k.shift = false;
    return k;
}

/* Parse a key combo like "ctrl+v", "enter", "shift+a" and send it */
static void send_key_combo(struct ei_device *dev, const char *combo, int delay_us) {
    char buf[256];
    strncpy(buf, combo, sizeof(buf) - 1);
    buf[sizeof(buf) - 1] = '\0';

    uint32_t modifiers[4];
    int nmod = 0;
    uint32_t keycode = 0;

    char *saveptr;
    char *tok = strtok_r(buf, "+", &saveptr);
    while (tok) {
        /* convert to lowercase for comparison */
        for (char *p = tok; *p; p++) *p = tolower(*p);

        char *next = strtok_r(NULL, "+", &saveptr);
        if (next == NULL) {
            /* last token is the key itself */
            if (strlen(tok) == 1) {
                struct keyinfo ki = char_to_key(tok[0]);
                keycode = ki.code;
                if (ki.shift && nmod < 4) modifiers[nmod++] = KEY_LEFTSHIFT;
            } else if (strcmp(tok, "enter") == 0 || strcmp(tok, "return") == 0) {
                keycode = KEY_ENTER;
            } else if (strcmp(tok, "tab") == 0) {
                keycode = KEY_TAB;
            } else if (strcmp(tok, "space") == 0) {
                keycode = KEY_SPACE;
            } else if (strcmp(tok, "esc") == 0 || strcmp(tok, "escape") == 0) {
                keycode = KEY_ESC;
            } else {
                fprintf(stderr, "ei-type: unknown key '%s'\n", tok);
                return;
            }
        } else {
            /* modifier */
            if (nmod >= 4) { tok = next; continue; }
            if (strcmp(tok, "ctrl") == 0 || strcmp(tok, "control") == 0) {
                modifiers[nmod++] = KEY_LEFTCTRL;
            } else if (strcmp(tok, "shift") == 0) {
                modifiers[nmod++] = KEY_LEFTSHIFT;
            } else if (strcmp(tok, "alt") == 0) {
                modifiers[nmod++] = KEY_LEFTALT;
            } else if (strcmp(tok, "super") == 0 || strcmp(tok, "meta") == 0) {
                modifiers[nmod++] = KEY_LEFTMETA;
            } else {
                fprintf(stderr, "ei-type: unknown modifier '%s'\n", tok);
                return;
            }
        }
        tok = next;
    }

    if (!keycode) return;

    /* press modifiers */
    for (int i = 0; i < nmod; i++) {
        ei_device_keyboard_key(dev, modifiers[i], true);
        ei_device_frame(dev, 0);
    }

    /* press and release key */
    ei_device_keyboard_key(dev, keycode, true);
    ei_device_frame(dev, 0);
    usleep(delay_us);
    ei_device_keyboard_key(dev, keycode, false);
    ei_device_frame(dev, 0);

    /* release modifiers in reverse */
    for (int i = nmod - 1; i >= 0; i--) {
        ei_device_keyboard_key(dev, modifiers[i], false);
        ei_device_frame(dev, 0);
    }

    ei_dispatch(ei_device_get_context(dev));
}

static void type_char(struct ei_device *dev, char c, int delay_us) {
    struct keyinfo ki = char_to_key(c);
    if (ki.code == 0) return; /* skip unmappable */

    if (ki.shift) {
        ei_device_keyboard_key(dev, KEY_LEFTSHIFT, true);
        ei_device_frame(dev, 0);
    }

    ei_device_keyboard_key(dev, ki.code, true);
    ei_device_frame(dev, 0);
    usleep(delay_us);

    ei_device_keyboard_key(dev, ki.code, false);
    ei_device_frame(dev, 0);

    if (ki.shift) {
        ei_device_keyboard_key(dev, KEY_LEFTSHIFT, false);
        ei_device_frame(dev, 0);
    }

    ei_dispatch(ei_device_get_context(dev));
}

static void usage(const char *prog) {
    fprintf(stderr, "Usage: %s [-d delay_ms] [-v] [--key combo]\n", prog);
    fprintf(stderr, "  -d N       inter-key delay in ms (default: 5)\n");
    fprintf(stderr, "  --key STR  send a key combo (e.g. ctrl+v, enter)\n");
    fprintf(stderr, "  -v         verbose debug output\n");
    fprintf(stderr, "  -h         show this help\n");
    fprintf(stderr, "\nReads text from stdin and types it into the focused window.\n");
}

int main(int argc, char *argv[]) {
    int delay_us = DEFAULT_DELAY_US;
    const char *key_combo = NULL;

    static struct option longopts[] = {
        {"key",     required_argument, NULL, 'k'},
        {"delay",   required_argument, NULL, 'd'},
        {"verbose", no_argument,       NULL, 'v'},
        {"help",    no_argument,       NULL, 'h'},
        {NULL, 0, NULL, 0}
    };

    int opt;
    while ((opt = getopt_long(argc, argv, "d:vh", longopts, NULL)) != -1) {
        switch (opt) {
            case 'k': key_combo = optarg; break;
            case 'd': delay_us = atoi(optarg) * 1000; break;
            case 'v': g_verbose = true; break;
            case 'h': usage(argv[0]); return 0;
            default:  usage(argv[0]); return 1;
        }
    }

    signal(SIGINT, sighandler);
    signal(SIGTERM, sighandler);

    /* Connect to KWin EIS via D-Bus */
    sd_bus *bus = NULL;
    int r = sd_bus_open_user(&bus);
    if (r < 0) {
        fprintf(stderr, "ei-type: failed to connect to session bus: %s\n", strerror(-r));
        return 1;
    }

    sd_bus_error error = SD_BUS_ERROR_NULL;
    sd_bus_message *reply = NULL;

    r = sd_bus_call_method(bus,
        "org.kde.KWin",
        "/org/kde/KWin/EIS/RemoteDesktop",
        "org.kde.KWin.EIS.RemoteDesktop",
        "connectToEIS",
        &error, &reply, "i", (int32_t)CAP_ALL);
    if (r < 0) {
        fprintf(stderr, "ei-type: D-Bus connectToEIS failed: %s\n",
                error.message ? error.message : strerror(-r));
        sd_bus_error_free(&error);
        sd_bus_unref(bus);
        return 1;
    }

    int fd = -1;
    int32_t cookie = 0;
    r = sd_bus_message_read(reply, "hi", &fd, &cookie);
    if (r < 0 || fd < 0) {
        fprintf(stderr, "ei-type: failed to read EIS fd from reply (r=%d, fd=%d)\n", r, fd);
        sd_bus_message_unref(reply);
        sd_bus_unref(bus);
        return 1;
    }
    DBG("got EIS fd=%d cookie=%d\n", fd, cookie);

    /* dup the fd — sd_bus_message_unref will close the original */
    int eis_fd = fcntl(fd, F_DUPFD_CLOEXEC, 3);
    if (eis_fd < 0) {
        fprintf(stderr, "ei-type: fcntl F_DUPFD_CLOEXEC failed: %s\n", strerror(errno));
        sd_bus_message_unref(reply);
        sd_bus_unref(bus);
        return 1;
    }
    DBG("dup'd fd=%d -> %d\n", fd, eis_fd);
    sd_bus_message_unref(reply);
    sd_bus_error_free(&error);

    /* Set up libei */
    struct ei *ei = ei_new_sender(NULL);
    if (!ei) {
        fprintf(stderr, "ei-type: ei_new_sender failed\n");
        close(eis_fd);
        sd_bus_unref(bus);
        return 1;
    }
    ei_configure_name(ei, "ei-type");

    r = ei_setup_backend_fd(ei, eis_fd);
    if (r < 0) {
        fprintf(stderr, "ei-type: ei_setup_backend_fd failed: %s\n", strerror(-r));
        ei_unref(ei);
        sd_bus_unref(bus);
        return 1;
    }
    DBG("ei_setup_backend_fd ok, ei_get_fd=%d\n", ei_get_fd(ei));

    /* Negotiate keyboard device via event loop */
    struct ei_device *kbd = NULL;
    bool ready = false;
    int timeout_count = 0;
    const int max_timeouts = 10; /* 10 * 500ms = 5s max */

    while (!ready && !g_quit && timeout_count < max_timeouts) {
        struct pollfd pfd = { .fd = ei_get_fd(ei), .events = POLLIN };
        int pr = poll(&pfd, 1, 500);
        if (pr < 0) {
            if (errno == EINTR) continue;
            fprintf(stderr, "ei-type: poll error: %s\n", strerror(errno));
            break;
        }

        if (pr == 0) {
            timeout_count++;
            DBG("poll timeout %d/%d\n", timeout_count, max_timeouts);
        }

        /* Always dispatch — data may have arrived between polls */
        ei_dispatch(ei);

        struct ei_event *ev;
        while ((ev = ei_get_event(ei)) != NULL) {
            enum ei_event_type type = ei_event_get_type(ev);
            DBG("event: %d\n", type);

            switch (type) {
            case EI_EVENT_CONNECT:
                DBG("connected to EIS\n");
                break;

            case EI_EVENT_SEAT_ADDED: {
                struct ei_seat *seat = ei_event_get_seat(ev);
                DBG("seat added, checking capabilities...\n");
                static const enum ei_device_capability all_caps[] = {
                    EI_DEVICE_CAP_KEYBOARD,
                    EI_DEVICE_CAP_POINTER,
                    EI_DEVICE_CAP_POINTER_ABSOLUTE,
                    EI_DEVICE_CAP_BUTTON,
                    EI_DEVICE_CAP_SCROLL,
                    EI_DEVICE_CAP_TOUCH,
                };
                bool has_kbd = false;
                for (int c = 0; c < 6; c++) {
                    bool has = ei_seat_has_capability(seat, all_caps[c]);
                    DBG("  cap %d: %s\n", all_caps[c], has ? "yes" : "no");
                    if (all_caps[c] == EI_DEVICE_CAP_KEYBOARD) has_kbd = has;
                }
                if (!has_kbd) {
                    fprintf(stderr, "ei-type: seat does not have keyboard capability\n");
                    g_quit = 1;
                    break;
                }
                /* Bind all supported capabilities (KWin provides them as a set) */
                ei_seat_bind_capabilities(seat,
                    EI_DEVICE_CAP_KEYBOARD,
                    EI_DEVICE_CAP_POINTER,
                    EI_DEVICE_CAP_POINTER_ABSOLUTE,
                    EI_DEVICE_CAP_BUTTON,
                    EI_DEVICE_CAP_SCROLL,
                    EI_DEVICE_CAP_TOUCH,
                    NULL);
                DBG("seat capabilities bound\n");
                break;
            }

            case EI_EVENT_DEVICE_ADDED: {
                struct ei_device *dev = ei_event_get_device(ev);
                if (ei_device_has_capability(dev, EI_DEVICE_CAP_KEYBOARD)) {
                    DBG("keyboard device added\n");
                    kbd = ei_device_ref(dev);
                }
                break;
            }

            case EI_EVENT_DEVICE_RESUMED:
                if (kbd) {
                    DBG("device resumed, starting emulation\n");
                    ei_device_start_emulating(kbd, 0);
                    ready = true;
                }
                break;

            case EI_EVENT_DISCONNECT:
                fprintf(stderr, "ei-type: disconnected by EIS\n");
                g_quit = 1;
                break;

            default:
                break;
            }

            ei_event_unref(ev);
        }
    }

    if (timeout_count >= max_timeouts) {
        fprintf(stderr, "ei-type: timeout waiting for EIS events (no response in 5s)\n");
    }

    if (!kbd || !ready) {
        fprintf(stderr, "ei-type: failed to get keyboard device\n");
        ei_unref(ei);
        sd_bus_unref(bus);
        return 1;
    }

    /* If --key mode, send the combo and exit */
    if (key_combo) {
        send_key_combo(kbd, key_combo, delay_us);
        usleep(delay_us);
        ei_device_unref(kbd);
        ei_unref(ei);
        sd_bus_unref(bus);
        return 0;
    }

    /* Read stdin and type each character */
    char buf[4096];
    while (!g_quit && fgets(buf, sizeof(buf), stdin)) {
        for (int i = 0; buf[i] && !g_quit; i++) {
            type_char(kbd, buf[i], delay_us);
            usleep(delay_us);
        }
    }

    /* Clean shutdown */
    ei_device_unref(kbd);
    ei_unref(ei);
    sd_bus_unref(bus);

    return 0;
}
