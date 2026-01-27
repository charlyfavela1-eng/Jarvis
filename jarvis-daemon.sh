#!/bin/bash
# JARVIS Daemon - Voice-activated, always listening
# Usage: ./jarvis-daemon.sh start|stop|status

JARVIS_DIR="/Users/michaelcaneyjr/Jarvis"
PID_FILE="/tmp/jarvis-listener.pid"
LOG_FILE="/tmp/jarvis-listener.log"

# Load environment
source ~/.zshrc 2>/dev/null

start_jarvis() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "JARVIS is already running (PID: $(cat $PID_FILE))"
        return
    fi

    echo "Starting JARVIS voice listener..."
    echo "Wake words: 'Jarvis', 'Hey Jarvis'"
    echo "Special: 'Daddy's home' (plays Back in Black)"

    cd "$JARVIS_DIR"
    source env/bin/activate
    export PYTHONPATH="$JARVIS_DIR:$JARVIS_DIR/jarviscli:$PYTHONPATH"
    export ELEVENLABS_API_KEY="${ELEVENLABS_API_KEY}"
    export SHODAN_API_KEY="${SHODAN_API_KEY}"

    # Run JARVIS listener in background
    nohup python3 jarvis-listener.py >> "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"

    sleep 3
    if kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "JARVIS is now listening (PID: $(cat $PID_FILE))"
        echo "Log file: $LOG_FILE"
        echo ""
        echo "Say 'Jarvis' or 'Hey Jarvis' to activate."
        echo "Say 'Daddy's home' for the special entrance."
    else
        echo "Failed to start JARVIS"
        rm -f "$PID_FILE"
    fi
}

stop_jarvis() {
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        if kill -0 "$PID" 2>/dev/null; then
            echo "Stopping JARVIS (PID: $PID)..."
            kill "$PID"
            rm -f "$PID_FILE"
            echo "JARVIS stopped."
        else
            echo "JARVIS is not running (stale PID file)"
            rm -f "$PID_FILE"
        fi
    else
        echo "JARVIS is not running"
    fi
}

status_jarvis() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "JARVIS is listening (PID: $(cat $PID_FILE))"
        echo "Say 'Jarvis' to activate."
    else
        echo "JARVIS is not running"
    fi
}

case "$1" in
    start)
        start_jarvis
        ;;
    stop)
        stop_jarvis
        ;;
    status)
        status_jarvis
        ;;
    restart)
        stop_jarvis
        sleep 1
        start_jarvis
        ;;
    log)
        tail -f "$LOG_FILE"
        ;;
    *)
        echo "Usage: $0 {start|stop|status|restart|log}"
        exit 1
        ;;
esac
