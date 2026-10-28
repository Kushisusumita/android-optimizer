#!/system/bin/sh
# Android Optimizer: compile the apps a person opens (icons), plus System UI and the home screen, but
# only those Android has not compiled itself. Already compiled apps (speed, speed-profile, everything)
# are left alone: on a real phone, recompiling them did not reliably make them start faster.
# The rest is compiled from its usage profile (speed-profile, forced now instead of waiting for the
# nightly job); an app with no profile yet stays at "verify" after that, so it gets full "speed".
# Output per app: "i/total OK pkg" compiled now, "i/total == pkg" already compiled,
# "i/total -- pkg" still not compiled (the firmware keeps some apps interpreted).

home=$(cmd package resolve-activity --brief -a android.intent.action.MAIN -c android.intent.category.HOME | tail -n 1 | cut -d/ -f1)
apps=$(cmd package query-activities --brief -a android.intent.action.MAIN -c android.intent.category.LAUNCHER | sed -n 's#^ *\([A-Za-z0-9_.]*\)/.*#\1#p' | sort -u)

# The compiler state of one package. No -m1 on the first grep: toybox grep then drops the context lines.
status() {
  dumpsys package "$1" | grep -A4 'Dexopt state:' | grep -o 'status=[a-z-]*' | head -n 1 | cut -d= -f2
}

set -- com.android.systemui $home $apps
total=$#
i=0
for p in "$@"; do
  i=$((i + 1))
  case "$(status "$p")" in
    speed|speed-profile|everything) echo "$i/$total == $p"; continue ;;
  esac
  cmd package compile -m speed-profile -f "$p" > /dev/null 2>&1
  [ "$(status "$p")" = verify ] && cmd package compile -m speed "$p" > /dev/null 2>&1
  # "Success" alone proves nothing: some firmware keeps an app at verify anyway. Trust the state.
  case "$(status "$p")" in
    speed|speed-profile|everything) echo "$i/$total OK $p" ;;
    *) echo "$i/$total -- $p" ;;
  esac
done
