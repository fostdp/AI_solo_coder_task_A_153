const API_BASE = 'http://localhost:3000';
const ALERT_MQTT_WS = 'ws://localhost:9001';

const LOD = { HIGH: 0, MEDIUM: 1, LOW: 2 };
const LOD_NAMES = ['HIGH', 'MEDIUM', 'LOW'];

let currentSpindle = 'SPD-001';
let simulationData = null;
let vibrationHistoryX = [];
let vibrationHistoryY = [];
const MAX_HISTORY = 200;
let alertList = [];
let stompClient = null;

const spindleGroup = new THREE.Group();
let spindleMesh = null;
let bearingMesh = null;
let yarnGroup = null;
let yarnLodObjects = [];
let currentLod = LOD.HIGH;
let motionBlurGroup = null;
let whirlGlow = null;
let scene, camera, renderer, controls;
let vibX = 0, vibY = 0;
let currentRpm = 0;
let currentVibAmp = 0;
let lastFrameTime = performance.now();
let frameTimeHistory = [];
let yarnBuildParams = null;

function initThreeScene() {
    const container = document.getElementById('three-container');
    const w = container.clientWidth;
    const h = container.clientHeight;

    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0a0e17);
    scene.fog = new THREE.FogExp2(0x0a0e17, 0.02);

    camera = new THREE.PerspectiveCamera(45, w / h, 0.1, 1000);
    camera.position.set(3, 4, 6);
    camera.lookAt(0, 2, 0);

    renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(w, h);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    container.appendChild(renderer.domElement);

    controls = new THREE.OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.05;
    controls.target.set(0, 2, 0);
    controls.update();

    const ambientLight = new THREE.AmbientLight(0x404060, 0.6);
    scene.add(ambientLight);

    const dirLight = new THREE.DirectionalLight(0xffffff, 0.8);
    dirLight.position.set(5, 10, 5);
    dirLight.castShadow = true;
    scene.add(dirLight);

    const pointLight1 = new THREE.PointLight(0x3b82f6, 0.5, 20);
    pointLight1.position.set(-3, 5, -3);
    scene.add(pointLight1);

    const pointLight2 = new THREE.PointLight(0x06b6d4, 0.4, 20);
    pointLight2.position.set(3, 3, 3);
    scene.add(pointLight2);

    const floorGeo = new THREE.PlaneGeometry(20, 20);
    const floorMat = new THREE.MeshStandardMaterial({ color: 0x111827, roughness: 0.9 });
    const floor = new THREE.Mesh(floorGeo, floorMat);
    floor.rotation.x = -Math.PI / 2;
    floor.receiveShadow = true;
    scene.add(floor);

    const gridHelper = new THREE.GridHelper(20, 40, 0x1a2332, 0x1a2332);
    scene.add(gridHelper);

    buildSpindleModel();
    scene.add(spindleGroup);

    window.addEventListener('resize', () => {
        const w2 = container.clientWidth;
        const h2 = container.clientHeight;
        camera.aspect = w2 / h2;
        camera.updateProjectionMatrix();
        renderer.setSize(w2, h2);
    });
}

function buildSpindleModel() {
    while (spindleGroup.children.length) {
        spindleGroup.remove(spindleGroup.children[0]);
    }

    const baseMat = new THREE.MeshStandardMaterial({ color: 0x4a3728, roughness: 0.7, metalness: 0.3 });
    const metalMat = new THREE.MeshStandardMaterial({ color: 0x8899aa, roughness: 0.3, metalness: 0.8 });
    const bearingMat = new THREE.MeshStandardMaterial({ color: 0xb8860b, roughness: 0.4, metalness: 0.6 });
    const whorlMat = new THREE.MeshStandardMaterial({ color: 0x6b4423, roughness: 0.6, metalness: 0.2 });

    const baseGeo = new THREE.CylinderGeometry(0.6, 0.7, 0.15, 32);
    const base = new THREE.Mesh(baseGeo, baseMat);
    base.position.y = 0.075;
    base.castShadow = true;
    spindleGroup.add(base);

    const basePillarGeo = new THREE.CylinderGeometry(0.08, 0.1, 0.4, 16);
    const basePillar = new THREE.Mesh(basePillarGeo, metalMat);
    basePillar.position.y = 0.35;
    basePillar.castShadow = true;
    spindleGroup.add(basePillar);

    const bearingGeo = new THREE.TorusGeometry(0.12, 0.04, 16, 32);
    bearingMesh = new THREE.Mesh(bearingGeo, bearingMat);
    bearingMesh.position.y = 0.55;
    bearingMesh.rotation.x = Math.PI / 2;
    bearingMesh.castShadow = true;
    spindleGroup.add(bearingMesh);

    const shaftGeo = new THREE.CylinderGeometry(0.02, 0.025, 3.0, 16);
    spindleMesh = new THREE.Mesh(shaftGeo, metalMat);
    spindleMesh.position.y = 2.05;
    spindleMesh.castShadow = true;
    spindleGroup.add(spindleMesh);

    const whorlGeo = new THREE.CylinderGeometry(0.25, 0.15, 0.12, 32);
    const whorl = new THREE.Mesh(whorlGeo, whorlMat);
    whorl.position.y = 1.0;
    whorl.castShadow = true;
    spindleGroup.add(whorl);

    const topBearingGeo = new THREE.TorusGeometry(0.06, 0.02, 12, 24);
    const topBearing = new THREE.Mesh(topBearingGeo, bearingMat);
    topBearing.position.y = 3.55;
    topBearing.rotation.x = Math.PI / 2;
    spindleGroup.add(topBearing);

    const topCapGeo = new THREE.ConeGeometry(0.04, 0.1, 16);
    const topCap = new THREE.Mesh(topCapGeo, metalMat);
    topCap.position.y = 3.6;
    spindleGroup.add(topCap);

    const glowGeo = new THREE.RingGeometry(0.08, 0.12, 32);
    const glowMat = new THREE.MeshBasicMaterial({
        color: 0xef4444,
        transparent: true,
        opacity: 0.0,
        side: THREE.DoubleSide,
    });
    whirlGlow = new THREE.Mesh(glowGeo, glowMat);
    whirlGlow.position.y = 2.05;
    whirlGlow.rotation.x = Math.PI / 2;
    spindleGroup.add(whirlGlow);

    buildYarnOnSpindle();
}

function buildYarnGeometry(params, lodLevel) {
    const yarnMat = new THREE.MeshStandardMaterial({
        color: 0xf5f0e0,
        roughness: 0.8,
        metalness: 0.0,
        emissive: 0x222211,
        emissiveIntensity: 0.1,
    });

    const { helixRadius, helixHeight, helixTurns } = params;

    if (lodLevel === LOD.HIGH) {
        const helixPoints = helixTurns * 16;
        const points = [];
        for (let i = 0; i <= helixPoints; i++) {
            const t = i / helixPoints;
            const angle = t * helixTurns * Math.PI * 2;
            const y = 1.1 + t * helixHeight;
            const r = helixRadius + 0.005 * Math.sin(t * 50);
            points.push(new THREE.Vector3(r * Math.cos(angle), y, r * Math.sin(angle)));
        }
        const curve = new THREE.CatmullRomCurve3(points);
        const tubeGeo = new THREE.TubeGeometry(curve, helixPoints * 2, 0.004, 6, false);
        return new THREE.Mesh(tubeGeo, yarnMat);
    } else if (lodLevel === LOD.MEDIUM) {
        const helixPoints = helixTurns * 4;
        const points = [];
        for (let i = 0; i <= helixPoints; i++) {
            const t = i / helixPoints;
            const angle = t * helixTurns * Math.PI * 2;
            const y = 1.1 + t * helixHeight;
            const r = helixRadius + 0.005 * Math.sin(t * 20);
            points.push(new THREE.Vector3(r * Math.cos(angle), y, r * Math.sin(angle)));
        }
        const lineGeo = new THREE.BufferGeometry().setFromPoints(points);
        const lineMat = new THREE.LineBasicMaterial({
            color: 0xf5f0e0,
            transparent: true,
            opacity: 0.85,
        });
        return new THREE.Line(lineGeo, lineMat);
    } else {
        const shellGeo = new THREE.CylinderGeometry(
            helixRadius + 0.01,
            helixRadius + 0.01,
            helixHeight,
            16, 1, true
        );
        const shellMat = new THREE.MeshStandardMaterial({
            color: 0xf0ead6,
            roughness: 0.95,
            side: THREE.DoubleSide,
            transparent: true,
            opacity: 0.4,
        });
        const shell = new THREE.Mesh(shellGeo, shellMat);
        shell.position.y = 1.1 + helixHeight / 2;
        return shell;
    }
}

function buildMotionBlurLines(params) {
    const group = new THREE.Group();
    const { helixRadius, helixHeight, helixTurns } = params;
    const blurLayers = 4;
    const maxAngleOffset = 0.15;

    for (let layer = 1; layer <= blurLayers; layer++) {
        const opacity = (1 - layer / (blurLayers + 1)) * 0.3;
        const helixPoints = Math.min(helixTurns * 3, 40);
        const points = [];
        const angleOffset = (layer / blurLayers) * maxAngleOffset;

        for (let i = 0; i <= helixPoints; i++) {
            const t = i / helixPoints;
            const angle = t * helixTurns * Math.PI * 2 + angleOffset;
            const y = 1.1 + t * helixHeight;
            const r = helixRadius;
            points.push(new THREE.Vector3(r * Math.cos(angle), y, r * Math.sin(angle)));
        }

        const lineGeo = new THREE.BufferGeometry().setFromPoints(points);
        const lineMat = new THREE.LineBasicMaterial({
            color: 0xf5f0e0,
            transparent: true,
            opacity: opacity,
        });
        const line = new THREE.Line(lineGeo, lineMat);
        group.add(line);

        const points2 = [];
        for (let i = 0; i <= helixPoints; i++) {
            const t = i / helixPoints;
            const angle = t * helixTurns * Math.PI * 2 - angleOffset;
            const y = 1.1 + t * helixHeight;
            const r = helixRadius;
            points2.push(new THREE.Vector3(r * Math.cos(angle), y, r * Math.sin(angle)));
        }
        const lineGeo2 = new THREE.BufferGeometry().setFromPoints(points2);
        const line2 = new THREE.Line(lineGeo2, lineMat.clone());
        group.add(line2);
    }

    return group;
}

function buildYarnOnSpindle() {
    if (yarnGroup) {
        spindleGroup.remove(yarnGroup);
    }
    yarnGroup = new THREE.Group();
    yarnLodObjects = [];
    motionBlurGroup = null;

    const twistPerMeter = simulationData ? simulationData.yarn_quality.twist_variance : 0;
    const helixRadius = 0.04 + twistPerMeter * 0.02;
    const helixHeight = 1.8;
    const helixTurns = 20 + twistPerMeter * 30;

    yarnBuildParams = { helixRadius, helixHeight, helixTurns };

    for (let lod = 0; lod < 3; lod++) {
        const obj = buildYarnGeometry(yarnBuildParams, lod);
        obj.visible = lod === LOD.HIGH;
        yarnLodObjects.push(obj);
        yarnGroup.add(obj);
    }

    const copGeo = new THREE.CylinderGeometry(helixRadius + 0.015, helixRadius + 0.015, helixHeight, 16, 1, true);
    const copMat = new THREE.MeshStandardMaterial({
        color: 0xf0ead6,
        roughness: 0.9,
        side: THREE.DoubleSide,
        transparent: true,
        opacity: 0.25,
    });
    const cop = new THREE.Mesh(copGeo, copMat);
    cop.position.y = 1.1 + helixHeight / 2;
    yarnGroup.add(cop);

    motionBlurGroup = buildMotionBlurLines(yarnBuildParams);
    motionBlurGroup.visible = false;
    yarnGroup.add(motionBlurGroup);

    currentLod = LOD.HIGH;
    updateLodVisibility();
    spindleGroup.add(yarnGroup);
}

function getCameraDistanceToSpindle() {
    const spindleCenter = new THREE.Vector3(0, 2.05, 0);
    return camera.position.distanceTo(spindleCenter);
}

function determineLod() {
    const distance = getCameraDistanceToSpindle();
    const rpm = currentRpm;

    let distanceLod = LOD.HIGH;
    if (distance > 8) distanceLod = LOD.LOW;
    else if (distance > 4) distanceLod = LOD.MEDIUM;

    let rpmLod = LOD.HIGH;
    if (rpm > 3500) rpmLod = LOD.LOW;
    else if (rpm > 2000) rpmLod = LOD.MEDIUM;

    let performanceLod = LOD.HIGH;
    if (frameTimeHistory.length > 10) {
        const avgFrameTime = frameTimeHistory.reduce((a, b) => a + b, 0) / frameTimeHistory.length;
        if (avgFrameTime > 33) performanceLod = LOD.LOW;
        else if (avgFrameTime > 20) performanceLod = LOD.MEDIUM;
    }

    return Math.max(distanceLod, rpmLod, performanceLod);
}

function updateLodVisibility() {
    const newLod = determineLod();

    if (newLod !== currentLod && yarnLodObjects.length === 3) {
        yarnLodObjects[currentLod].visible = false;
        yarnLodObjects[newLod].visible = true;
        currentLod = newLod;

        if (motionBlurGroup) {
            motionBlurGroup.visible = currentLod !== LOD.HIGH && currentRpm > 1500;
        }
    }
}

function updateSpindleVibration(time) {
    if (!spindleMesh || !simulationData) return;

    const vib = simulationData.vibration;
    const freq = currentRpm / 60.0;
    const omega = freq * Math.PI * 2;
    const scale = Math.min(vib.total_displacement * 50, 0.5);

    const whirlMod = vib.whirl_instability ? 1.5 : 1.0;
    const whirlOmega = omega * vib.whirl_ratio * whirlMod;

    vibX = scale * Math.cos(omega * time) + scale * 0.3 * Math.cos(whirlOmega * time);
    vibY = scale * Math.sin(omega * time) + scale * 0.3 * Math.sin(whirlOmega * time);

    spindleMesh.position.x = vibX;
    spindleMesh.position.z = vibY;

    if (yarnGroup) {
        yarnGroup.position.x = vibX * 0.5;
        yarnGroup.position.z = vibY * 0.5;
    }

    if (whirlGlow) {
        const targetOpacity = vib.whirl_instability ? 0.6 : 0.0;
        whirlGlow.material.opacity += (targetOpacity - whirlGlow.material.opacity) * 0.1;
        whirlGlow.rotation.z += omega * 0.016;
    }

    const rotationSpeed = currentRpm / 60.0 * Math.PI * 2 * 0.016;
    spindleMesh.rotation.y += rotationSpeed;
}

function animate() {
    requestAnimationFrame(animate);

    const now = performance.now();
    const frameTime = now - lastFrameTime;
    lastFrameTime = now;

    frameTimeHistory.push(frameTime);
    if (frameTimeHistory.length > 20) frameTimeHistory.shift();

    const time = now / 1000;
    updateSpindleVibration(time);
    updateLodVisibility();
    controls.update();
    renderer.render(scene, camera);
}

function initVibrationCanvas() {
    const canvas = document.getElementById('vibration-canvas');
    canvas.width = canvas.offsetWidth * 2;
    canvas.height = 240;
}

function drawVibrationWaveform() {
    const canvas = document.getElementById('vibration-canvas');
    const ctx = canvas.getContext('2d');
    const w = canvas.width;
    const h = canvas.height;

    ctx.fillStyle = '#111827';
    ctx.fillRect(0, 0, w, h);

    ctx.strokeStyle = '#1a2332';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, h / 2);
    ctx.lineTo(w, h / 2);
    ctx.stroke();

    for (let i = 1; i <= 3; i++) {
        ctx.beginPath();
        ctx.moveTo(0, h / 2 + (h / 8) * i);
        ctx.lineTo(w, h / 2 + (h / 8) * i);
        ctx.moveTo(0, h / 2 - (h / 8) * i);
        ctx.lineTo(w, h / 2 - (h / 8) * i);
        ctx.strokeStyle = 'rgba(42,58,78,0.3)';
        ctx.stroke();
    }

    if (vibrationHistoryX.length > 1) {
        ctx.beginPath();
        ctx.strokeStyle = '#3b82f6';
        ctx.lineWidth = 2;
        for (let i = 0; i < vibrationHistoryX.length; i++) {
            const x = (i / MAX_HISTORY) * w;
            const y = h / 2 - vibrationHistoryX[i] * h * 0.4;
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        }
        ctx.stroke();

        ctx.beginPath();
        ctx.strokeStyle = '#f59e0b';
        ctx.lineWidth = 2;
        for (let i = 0; i < vibrationHistoryY.length; i++) {
            const x = (i / MAX_HISTORY) * w;
            const y = h / 2 - vibrationHistoryY[i] * h * 0.4;
            if (i === 0) ctx.moveTo(x, y);
            else ctx.lineTo(x, y);
        }
        ctx.stroke();
    }

    ctx.font = '20px sans-serif';
    ctx.fillStyle = '#3b82f6';
    ctx.fillText('X', 10, 24);
    ctx.fillStyle = '#f59e0b';
    ctx.fillText('Y', 40, 24);

    if (simulationData && simulationData.vibration.whirl_instability) {
        ctx.font = 'bold 24px sans-serif';
        ctx.fillStyle = '#ef4444';
        ctx.fillText('⚠ 油膜涡动不稳定', w - 280, 30);
    }
}

function updateSensorDisplay(data) {
    document.getElementById('val-rpm').textContent = data.rpm.toFixed(0);
    document.getElementById('val-vib').textContent = data.vibration_amplitude.toFixed(3);
    document.getElementById('val-temp').textContent = data.temperature.toFixed(1);
    document.getElementById('val-twist').textContent = data.twist_per_meter.toFixed(0);

    currentRpm = data.rpm;
    currentVibAmp = data.vibration_amplitude;
}

function updateSimulationDisplay(sim) {
    document.getElementById('overlay-critical').textContent = sim.vibration.critical_rpm.toFixed(0) + ' RPM';
    document.getElementById('overlay-displacement').textContent = sim.vibration.total_displacement.toFixed(4) + ' mm';
    document.getElementById('overlay-whirl').textContent = sim.vibration.whirl_ratio.toFixed(2);

    const uniformity = Math.max(0, Math.min(100, sim.yarn_quality.predicted_uniformity));
    const strength = Math.max(0, Math.min(30, sim.yarn_quality.predicted_strength));
    const impact = Math.max(0, Math.min(1, sim.yarn_quality.vibration_impact_factor));

    document.getElementById('val-uniformity').textContent = uniformity.toFixed(1) + '%';
    document.getElementById('val-strength').textContent = strength.toFixed(1) + ' cN/tex';
    document.getElementById('val-impact').textContent = impact.toFixed(3);

    document.getElementById('bar-uniformity').style.width = uniformity + '%';
    document.getElementById('bar-strength').style.width = (strength / 30 * 100) + '%';
    document.getElementById('bar-impact').style.width = (impact * 100) + '%';

    const vx = sim.vibration.vibration_x;
    const vy = sim.vibration.vibration_y;
    vibrationHistoryX.push(vx > 0 ? Math.min(vx * 100, 1) : Math.max(vx * 100, -1));
    vibrationHistoryY.push(vy > 0 ? Math.min(vy * 100, 1) : Math.max(vy * 100, -1));
    if (vibrationHistoryX.length > MAX_HISTORY) vibrationHistoryX.shift();
    if (vibrationHistoryY.length > MAX_HISTORY) vibrationHistoryY.shift();

    drawVibrationWaveform();

    const wearEl = document.getElementById('val-wear');
    if (wearEl && sim.yarn_quality.wear_coefficient !== undefined) {
        wearEl.textContent = (sim.yarn_quality.wear_coefficient * 100).toFixed(2) + '%';
    }

    const lodEl = document.getElementById('val-lod');
    if (lodEl) {
        lodEl.textContent = LOD_NAMES[currentLod] + ' (' + Math.round(1000 / (frameTimeHistory[frameTimeHistory.length - 1] || 16)) + ' FPS)';
    }
}

function addAlerts(alerts) {
    const container = document.getElementById('alert-list');
    const emptyEl = container.querySelector('.empty-alerts');
    if (emptyEl) emptyEl.remove();

    for (const alert of alerts) {
        const item = document.createElement('div');
        item.className = 'alert-item';
        const iconClass = alert.severity === 'critical' ? 'critical' : 'warning';
        const titleMap = {
            vibration_overload: '振动超限',
            twist_uneven: '捻度不均',
            critical_speed: '临界转速',
            temperature_high: '温度过高',
        };
        item.innerHTML = `
            <div class="alert-icon ${iconClass}"></div>
            <div class="alert-content">
                <div class="alert-title">${titleMap[alert.alert_type] || alert.alert_type} · ${alert.severity === 'critical' ? '严重' : '警告'}</div>
                <div class="alert-msg">${alert.message}</div>
                <div class="alert-time">${new Date(alert.timestamp).toLocaleTimeString('zh-CN')}</div>
            </div>
        `;
        container.insertBefore(item, container.firstChild);
        alertList.unshift(alert);
    }

    document.getElementById('alert-count').textContent = alertList.length;
}

async function fetchSensorData() {
    try {
        const resp = await fetch(`${API_BASE}/api/sensor-data?spindle_id=${currentSpindle}&limit=1`);
        const json = await resp.json();
        if (json.data && json.data.length > 0) {
            const row = json.data[0];
            updateSensorDisplay({
                rpm: parseFloat(row.rpm),
                vibration_amplitude: parseFloat(row.vibration_amplitude),
                temperature: parseFloat(row.temperature),
                twist_per_meter: parseFloat(row.twist_per_meter),
            });
        }
        document.getElementById('connection-status').textContent = '已连接';
    } catch (e) {
        console.error('Fetch sensor data error:', e);
        document.getElementById('connection-status').textContent = '连接失败';
    }
}

async function runSimulation() {
    try {
        const sensorResp = await fetch(`${API_BASE}/api/sensor-data?spindle_id=${currentSpindle}&limit=1`);
        const sensorJson = await sensorResp.json();

        let rpm = 1500, vibAmp = 0.15, temp = 35, twist = 800;
        if (sensorJson.data && sensorJson.data.length > 0) {
            const row = sensorJson.data[0];
            rpm = parseFloat(row.rpm);
            vibAmp = parseFloat(row.vibration_amplitude);
            temp = parseFloat(row.temperature);
            twist = parseFloat(row.twist_per_meter);
        }

        const simResp = await fetch(`${API_BASE}/api/simulate`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                spindle_id: currentSpindle,
                rpm: rpm,
                vibration_amplitude: vibAmp,
                temperature: temp,
                twist_per_meter: twist,
            }),
        });

        simulationData = await simResp.json();
        updateSimulationDisplay(simulationData);

        if (simulationData.alerts && simulationData.alerts.length > 0) {
            addAlerts(simulationData.alerts);
        }

        buildYarnOnSpindle();
    } catch (e) {
        console.error('Simulation error:', e);
    }
}

function connectAlertWebSocket() {
    try {
        const ws = new WebSocket(ALERT_MQTT_WS);
        ws.onopen = () => console.log('Alert WebSocket connected');
        ws.onmessage = (event) => {
            try {
                const alert = JSON.parse(event.data);
                addAlerts([alert]);
            } catch (e) {}
        };
        ws.onerror = (e) => console.warn('Alert WS error', e);
        ws.onclose = () => setTimeout(connectAlertWebSocket, 5000);
    } catch (e) {
        setTimeout(connectAlertWebSocket, 5000);
    }
}

function generateDemoData() {
    const rpm = 1500 + 200 * Math.sin(Date.now() / 2000) + (Math.random() - 0.5) * 60;
    const vibAmp = 0.15 + 0.1 * Math.sin(Date.now() / 3000) + Math.random() * 0.02;
    const temp = 35 + rpm / 1000 * 5 + (Math.random() - 0.5) * 2;
    const twist = 800 + 50 * Math.sin(Date.now() / 5000) + (Math.random() - 0.5) * 40;

    return { rpm, vibration_amplitude: vibAmp, temperature: temp, twist_per_meter: twist };
}

function computeLocalSimulation(rpm, vibAmp, temperature, twist) {
    const m = 0.5;
    const L = 0.3;
    const d = 0.008;
    const E = 210e9;
    const I_shaft = Math.PI * Math.pow(d, 4) / 64;
    const k_shaft = 48 * E * I_shaft / Math.pow(L, 3);
    const omega_cr = Math.sqrt(k_shaft / m);
    const critical_rpm = omega_cr * 60 / (2 * Math.PI);

    const omega = rpm * 2 * Math.PI / 60;
    const r = omega / omega_cr;
    const e_unbalance = 0.0001;
    const zeta = 0.02;
    const unbalance_response = e_unbalance * r * r / Math.sqrt(Math.pow(1 - r * r, 2) + Math.pow(2 * zeta * r, 2));

    const mu = 0.01;
    const bL = 0.02;
    const bD = 0.016;
    const bR = 0.008;
    const c = 0.00005;
    const g = 9.81;
    const W = m * g;
    const n_rps = rpm / 60;
    const S = (mu * n_rps * bL * bD) / W * Math.pow(bR / c, 2);
    const eccentricity_ratio = 1 - 1 / (2 * S + 1);

    const eps = Math.min(0.95, Math.max(0.01, eccentricity_ratio));
    const k0 = mu * omega * bL * Math.pow(bR / c, 3) / (2 * Math.PI);
    const c0 = mu * bL * Math.pow(bR / c, 3) / (2 * Math.PI);
    const k_xx = k0 * (1 + 2 * eps * eps);
    const k_yy = k0 * (1 - 2 * eps * eps);
    const c_xx_linear = c0 * (1 + eps * eps);
    const c_yy_linear = c0 * (1 - eps * eps);

    const theta = omega * 0.1;
    const denom = 1 + eps * Math.cos(theta);
    const pressure_peak = Math.abs((mu * omega * bR * bR / (c * c)) * eps * Math.sin(theta) * (2/3) / (denom * denom));

    let threshold = 0.55;
    if (eps < 0.3) threshold = 0.45;
    else if (eps < 0.6) threshold = 0.5;
    const whirl_instability = r > threshold && eps > 0.2;
    let whirl_ratio = 0.5;
    if (whirl_instability) {
        const factor = 1 + 0.3 * (r - threshold) / Math.max(0.01, 1 - threshold);
        whirl_ratio = 0.5 * factor;
    }

    const F0 = m * e_unbalance * omega * omega;
    let vib_x_linear = F0 / Math.sqrt(Math.pow(k_xx - m * omega * omega, 2) + Math.pow(c_xx_linear * omega, 2));
    let vib_y_linear = F0 / Math.sqrt(Math.pow(k_yy - m * omega * omega, 2) + Math.pow(c_yy_linear * omega, 2));

    const alpha_nonlinear = 5e6;
    const disp_x = Math.min(Math.abs(vib_x_linear), 0.001);
    const disp_y = Math.min(Math.abs(vib_y_linear), 0.001);
    const c_xx = c_xx_linear * (1 + alpha_nonlinear * disp_x * disp_x);
    const c_yy = c_yy_linear * (1 + alpha_nonlinear * disp_y * disp_y);

    let vib_x = F0 / Math.sqrt(Math.pow(k_xx - m * omega * omega, 2) + Math.pow(c_xx * omega, 2));
    let vib_y = F0 / Math.sqrt(Math.pow(k_yy - m * omega * omega, 2) + Math.pow(c_yy * omega, 2));

    let total_disp = Math.sqrt(vib_x * vib_x + vib_y * vib_y);
    if (whirl_instability) {
        const growth = 1 + 2.5 * Math.max(0, r - 0.55) * Math.max(0, eps - 0.2) * 10;
        total_disp *= Math.min(growth, 8.0);
        const scale = total_disp / Math.max(1e-12, Math.sqrt(vib_x_linear * vib_x_linear + vib_y_linear * vib_y_linear));
        vib_x *= scale;
        vib_y *= scale;
    }

    const nonlinear_damping_factor = c_xx / Math.max(1e-12, c_xx_linear);
    const k_pi = Math.PI * Math.pow(1 - eps * eps, -1.5);
    const nl_fx = -mu * omega * Math.pow(bL, 3) * bR / (c * c) * eps * (2 + eps * eps) * k_pi / (4 * Math.pow(1 - eps * eps, 2));
    const nl_fy = mu * omega * Math.pow(bL, 3) * bR / (c * c) * Math.PI * eps / (2 * Math.pow(1 - eps * eps, 2));

    const vibration = {
        critical_rpm, unbalance_response,
        oil_film_stiffness_x: k_xx, oil_film_stiffness_y: k_yy,
        oil_film_damping_x: c_xx, oil_film_damping_y: c_yy,
        whirl_ratio, eccentricity_ratio: eps,
        vibration_x: vib_x, vibration_y: vib_y,
        total_displacement: total_disp,
        phase_angle: Math.atan2(vib_y, vib_x),
        nonlinear_force_x: nl_fx,
        nonlinear_force_y: nl_fy,
        whirl_instability,
        nonlinear_damping_factor,
        oil_film_pressure_peak: pressure_peak,
    };

    const target_twist = 800;
    const twist_variance = Math.abs(twist - target_twist) / target_twist;
    const predicted_uniformity = Math.max(0, 95 - 0.8 * vibAmp - 0.3 * twist_variance - 0.05 * vibAmp * twist_variance + (Math.random() - 0.5));
    const twist_factor = twist / 100;
    const predicted_strength = Math.max(0, 15 + 0.02 * twist_factor - 1.5 * vibAmp - 0.00001 * twist_factor * twist_factor + (Math.random() - 0.5));
    const vibration_impact_factor = 1 - Math.exp(-2 * vibAmp);

    const yarn_quality = {
        predicted_uniformity,
        predicted_strength,
        twist_variance,
        vibration_impact_factor,
        wear_coefficient: 0.0,
        calibration_error: 0.0,
        sample_count: 0,
        beta0: 95.0,
        beta1: -0.8,
        alpha0: 15.0,
        alpha1: 0.02,
    };

    const alerts = [];
    const now = new Date().toISOString();
    if (vibAmp > 1.0) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'vibration_overload', severity: 'critical', message: `振动幅值 ${vibAmp.toFixed(3)} mm 超过严重阈值 1.0 mm`, value: vibAmp, threshold: 1.0 });
    } else if (vibAmp > 0.5) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'vibration_overload', severity: 'warning', message: `振动幅值 ${vibAmp.toFixed(3)} mm 超过警告阈值 0.5 mm`, value: vibAmp, threshold: 0.5 });
    }
    if (twist_variance > 0.2) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'twist_uneven', severity: 'critical', message: `捻度偏差 ${twist_variance.toFixed(3)} 超过严重阈值 0.2`, value: twist_variance, threshold: 0.2 });
    } else if (twist_variance > 0.1) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'twist_uneven', severity: 'warning', message: `捻度偏差 ${twist_variance.toFixed(3)} 超过警告阈值 0.1`, value: twist_variance, threshold: 0.1 });
    }
    if (critical_rpm > 0 && Math.abs(rpm - critical_rpm) / critical_rpm <= 0.1) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'critical_speed', severity: 'critical', message: `转速 ${rpm.toFixed(0)} 接近临界转速 ${critical_rpm.toFixed(0)}`, value: rpm, threshold: critical_rpm });
    }
    if (temperature > 80) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'temperature_high', severity: 'critical', message: `温度 ${temperature.toFixed(1)}°C 超过严重阈值 80°C`, value: temperature, threshold: 80 });
    } else if (temperature > 60) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'temperature_high', severity: 'warning', message: `温度 ${temperature.toFixed(1)}°C 超过警告阈值 60°C`, value: temperature, threshold: 60 });
    }
    if (whirl_instability) {
        alerts.push({ timestamp: now, spindle_id: currentSpindle, alert_type: 'oil_whirl', severity: 'critical', message: `检测到油膜涡动不稳定，涡动比 ${whirl_ratio.toFixed(3)}`, value: whirl_ratio, threshold: 0.55 });
    }

    return { vibration, yarn_quality, alerts };
}

async function demoLoop() {
    const data = generateDemoData();
    updateSensorDisplay(data);

    try {
        const simResp = await fetch(`${API_BASE}/api/simulate`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                spindle_id: currentSpindle,
                rpm: data.rpm,
                vibration_amplitude: data.vibration_amplitude,
                temperature: data.temperature,
                twist_per_meter: data.twist_per_meter,
            }),
        });
        simulationData = await simResp.json();
        updateSimulationDisplay(simulationData);

        if (simulationData.alerts && simulationData.alerts.length > 0) {
            addAlerts(simulationData.alerts);
        }

        if (!yarnBuildParams ||
            Math.abs(yarnBuildParams.helixRadius - (0.04 + simulationData.yarn_quality.twist_variance * 0.02)) > 0.001) {
            buildYarnOnSpindle();
        }
        document.getElementById('connection-status').textContent = '已连接(实时)';
    } catch (e) {
        const sim = computeLocalSimulation(data.rpm, data.vibration_amplitude, data.temperature, data.twist_per_meter);
        simulationData = sim;
        updateSimulationDisplay(sim);

        if (sim.alerts.length > 0) {
            addAlerts(sim.alerts);
        }

        if (!yarnBuildParams ||
            Math.abs(yarnBuildParams.helixRadius - (0.04 + sim.yarn_quality.twist_variance * 0.02)) > 0.001) {
            buildYarnOnSpindle();
        }
        document.getElementById('connection-status').textContent = '本地模式';
    }
}

document.getElementById('spindle-select').addEventListener('change', (e) => {
    currentSpindle = e.target.value;
    vibrationHistoryX = [];
    vibrationHistoryY = [];
    yarnBuildParams = null;
});

document.getElementById('simulate-btn').addEventListener('click', runSimulation);

window.addEventListener('load', () => {
    const overlay = document.querySelector('.three-overlay');
    if (overlay) {
        const wearBadge = document.createElement('div');
        wearBadge.className = 'overlay-badge';
        wearBadge.innerHTML = '<span class="label">磨损系数</span><span class="value" id="val-wear" style="color:var(--accent-red)">--%</span>';
        overlay.appendChild(wearBadge);

        const lodBadge = document.createElement('div');
        lodBadge.className = 'overlay-badge';
        lodBadge.innerHTML = '<span class="label">渲染</span><span class="value" id="val-lod" style="color:var(--accent-green)">--</span>';
        overlay.appendChild(lodBadge);
    }

    initThreeScene();
    initVibrationCanvas();
    animate();

    connectAlertWebSocket();

    demoLoop();
    setInterval(demoLoop, 2000);
});
