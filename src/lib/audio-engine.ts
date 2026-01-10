export interface AudioDeviceInfo {
  id: string;
  name: string;
  type: "input" | "output";
  isDefault: boolean;
  channels: number;
  sampleRate: number;
  latency: number;
}

export interface AudioStreamNode {
  id: string;
  deviceId: string;
  mediaStream?: MediaStream;
  audioNode?: AudioNode;
  gainNode?: GainNode;
  analyserNode?: AnalyserNode;
  isActive: boolean;
  volume: number;
  levels: Float32Array<ArrayBuffer>;
}

export interface AudioRoute {
  id: string;
  sourceNodeId: string;
  targetNodeId: string;
  gainNode: GainNode;
  isActive: boolean;
  volume: number;
  effects: AudioEffect[];
}

export interface AudioEffect {
  id: string;
  type: "equalizer" | "compressor" | "reverb" | "delay" | "filter";
  node: AudioNode;
  enabled: boolean;
  parameters: Record<string, number>;
}

export class AudioRoutingEngine {
  private audioContext: AudioContext | null = null;
  private inputNodes: Map<string, AudioStreamNode> = new Map();
  private outputNodes: Map<string, AudioStreamNode> = new Map();
  private routes: Map<string, AudioRoute> = new Map();
  private masterGainNode: GainNode | null = null;
  private isInitialized = false;
  private animationFrameId: number | null = null;

  constructor() {
    this.initializeAudioContext();
  }

  private async initializeAudioContext() {
    try {
      this.audioContext = new (window.AudioContext ||
        (window as any).webkitAudioContext)();

      if (this.audioContext.state === "suspended") {
        await this.audioContext.resume();
      }

      this.masterGainNode = this.audioContext.createGain();
      this.masterGainNode.connect(this.audioContext.destination);

      this.isInitialized = true;
      console.log("[v0] Audio context initialized successfully");
    } catch (error) {
      console.error("[v0] Failed to initialize audio context:", error);
    }
  }

  async getAvailableDevices(): Promise<AudioDeviceInfo[]> {
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      console.log("[v0] Raw device enumeration:", devices);

      const audioDevices: AudioDeviceInfo[] = [];

      for (const device of devices) {
        console.log(
          "[v0] Processing device:",
          device.kind,
          device.label,
          device.deviceId
        );

        if (device.kind === "audioinput" || device.kind === "audiooutput") {
          const audioDevice = {
            id: device.deviceId,
            name:
              device.label ||
              `${
                device.kind === "audioinput" ? "Microphone" : "Speaker"
              } ${device.deviceId.slice(0, 8)}`,
            type:
              device.kind === "audioinput"
                ? "input"
                : ("output" as "input" | "output"),
            isDefault: device.deviceId === "default",
            channels: 2, // Default to stereo
            sampleRate: this.audioContext?.sampleRate || 44100,
            latency: 0.02, // Default 20ms latency
          };

          console.log("[v0] Mapped audio device:", audioDevice);
          audioDevices.push(audioDevice);
        }
      }

      console.log("[v0] Final audio devices list:", audioDevices);
      return audioDevices;
    } catch (error) {
      console.error("[v0] Failed to enumerate devices:", error);
      return [];
    }
  }

  async createInputNode(deviceId: string): Promise<AudioStreamNode | null> {
    if (!this.audioContext || !this.isInitialized) {
      console.error("[v0] Audio context not initialized");
      return null;
    }

    try {
      const constraints: MediaStreamConstraints = {
        audio: {
          deviceId: deviceId === "default" ? undefined : { exact: deviceId },
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        },
      };

      const mediaStream = await navigator.mediaDevices.getUserMedia(
        constraints
      );
      const sourceNode = this.audioContext.createMediaStreamSource(mediaStream);
      const gainNode = this.audioContext.createGain();
      const analyserNode = this.audioContext.createAnalyser();

      // Configure analyser for level monitoring
      analyserNode.fftSize = 256;
      analyserNode.smoothingTimeConstant = 0.8;

      // Connect nodes
      sourceNode.connect(gainNode);
      gainNode.connect(analyserNode);

      const streamNode: AudioStreamNode = {
        id: `input-${deviceId}`,
        deviceId,
        mediaStream,
        audioNode: sourceNode,
        gainNode,
        analyserNode,
        isActive: true,
        volume: 75,
        levels: new Float32Array(analyserNode.frequencyBinCount),
      };

      this.inputNodes.set(streamNode.id, streamNode);
      console.log("[v0] Created input node for device:", deviceId);

      return streamNode;
    } catch (error) {
      console.error("[v0] Failed to create input node:", error);
      return null;
    }
  }

  async createOutputNode(deviceId: string): Promise<AudioStreamNode | null> {
    if (!this.audioContext || !this.isInitialized) {
      console.error("[v0] Audio context not initialized");
      return null;
    }

    try {
      const gainNode = this.audioContext.createGain();
      const analyserNode = this.audioContext.createAnalyser();

      // Configure analyser
      analyserNode.fftSize = 256;
      analyserNode.smoothingTimeConstant = 0.8;

      // Connect to master output
      gainNode.connect(analyserNode);
      analyserNode.connect(this.masterGainNode!);

      const streamNode: AudioStreamNode = {
        id: `output-${deviceId}`,
        deviceId,
        audioNode: gainNode,
        gainNode,
        analyserNode,
        isActive: true,
        volume: 75,
        levels: new Float32Array(analyserNode.frequencyBinCount),
      };

      this.outputNodes.set(streamNode.id, streamNode);
      console.log("[v0] Created output node for device:", deviceId);

      return streamNode;
    } catch (error) {
      console.error("[v0] Failed to create output node:", error);
      return null;
    }
  }

  createRoute(sourceNodeId: string, targetNodeId: string): AudioRoute | null {
    if (!this.audioContext || !this.isInitialized) {
      console.error("[v0] Audio context not initialized");
      return null;
    }

    const sourceNode = this.inputNodes.get(sourceNodeId);
    const targetNode = this.outputNodes.get(targetNodeId);

    if (!sourceNode || !targetNode) {
      console.error("[v0] Source or target node not found");
      return null;
    }

    try {
      const routeGainNode = this.audioContext.createGain();
      routeGainNode.gain.value = 0.75; // Default 75% volume

      // Connect source to route gain, then to target
      sourceNode.gainNode!.connect(routeGainNode);
      routeGainNode.connect(targetNode.gainNode!);

      const route: AudioRoute = {
        id: `${sourceNodeId}-${targetNodeId}`,
        sourceNodeId,
        targetNodeId,
        gainNode: routeGainNode,
        isActive: true,
        volume: 75,
        effects: [],
      };

      this.routes.set(route.id, route);
      console.log("[v0] Created audio route:", route.id);

      return route;
    } catch (error) {
      console.error("[v0] Failed to create route:", error);
      return null;
    }
  }

  removeRoute(routeId: string): boolean {
    const route = this.routes.get(routeId);
    if (!route) return false;

    try {
      // Disconnect all nodes in the route
      route.gainNode.disconnect();

      // Remove effects
      route.effects.forEach((effect) => {
        effect.node.disconnect();
      });

      this.routes.delete(routeId);
      console.log("[v0] Removed audio route:", routeId);
      return true;
    } catch (error) {
      console.error("[v0] Failed to remove route:", error);
      return false;
    }
  }

  setRouteVolume(routeId: string, volume: number): boolean {
    const route = this.routes.get(routeId);
    if (!route) return false;

    try {
      const normalizedVolume = Math.max(0, Math.min(100, volume)) / 100;
      route.gainNode.gain.setValueAtTime(
        normalizedVolume,
        this.audioContext!.currentTime
      );
      route.volume = volume;
      return true;
    } catch (error) {
      console.error("[v0] Failed to set route volume:", error);
      return false;
    }
  }

  setNodeVolume(nodeId: string, volume: number): boolean {
    const node = this.inputNodes.get(nodeId) || this.outputNodes.get(nodeId);
    if (!node || !node.gainNode) return false;

    try {
      const normalizedVolume = Math.max(0, Math.min(100, volume)) / 100;
      node.gainNode.gain.setValueAtTime(
        normalizedVolume,
        this.audioContext!.currentTime
      );
      node.volume = volume;
      return true;
    } catch (error) {
      console.error("[v0] Failed to set node volume:", error);
      return false;
    }
  }

  setMasterVolume(volume: number): boolean {
    if (!this.masterGainNode) return false;

    try {
      const normalizedVolume = Math.max(0, Math.min(100, volume)) / 100;
      this.masterGainNode.gain.setValueAtTime(
        normalizedVolume,
        this.audioContext!.currentTime
      );
      return true;
    } catch (error) {
      console.error("[v0] Failed to set master volume:", error);
      return false;
    }
  }

  getAudioLevels(): Map<string, Float32Array> {
    const levels = new Map<string, Float32Array>();

    // Get levels from all active nodes
    for (const [nodeId, node] of [...this.inputNodes, ...this.outputNodes]) {
      if (node.isActive && node.analyserNode) {
        node.analyserNode.getFloatFrequencyData(node.levels);
        levels.set(nodeId, new Float32Array(node.levels));
      }
    }

    return levels;
  }

  startLevelMonitoring(
    callback: (levels: Map<string, Float32Array>) => void
  ): void {
    const updateLevels = () => {
      const levels = this.getAudioLevels();
      callback(levels);
      this.animationFrameId = requestAnimationFrame(updateLevels);
    };

    updateLevels();
  }

  stopLevelMonitoring(): void {
    if (this.animationFrameId) {
      cancelAnimationFrame(this.animationFrameId);
      this.animationFrameId = null;
    }
  }

  addEffect(
    routeId: string,
    effectType: AudioEffect["type"]
  ): AudioEffect | null {
    const route = this.routes.get(routeId);
    if (!route || !this.audioContext) return null;

    try {
      let effectNode: AudioNode;

      switch (effectType) {
        case "equalizer":
          // Create a 3-band EQ using BiquadFilters
          const lowShelf = this.audioContext.createBiquadFilter();
          lowShelf.type = "lowshelf";
          lowShelf.frequency.value = 320;

          const midPeaking = this.audioContext.createBiquadFilter();
          midPeaking.type = "peaking";
          midPeaking.frequency.value = 1000;
          midPeaking.Q.value = 0.5;

          const highShelf = this.audioContext.createBiquadFilter();
          highShelf.type = "highshelf";
          highShelf.frequency.value = 3200;

          lowShelf.connect(midPeaking);
          midPeaking.connect(highShelf);
          effectNode = lowShelf;
          break;

        case "compressor":
          const compressor = this.audioContext.createDynamicsCompressor();
          compressor.threshold.value = -24;
          compressor.knee.value = 30;
          compressor.ratio.value = 12;
          compressor.attack.value = 0.003;
          compressor.release.value = 0.25;
          effectNode = compressor;
          break;

        case "delay":
          const delay = this.audioContext.createDelay(1.0);
          delay.delayTime.value = 0.3;
          effectNode = delay;
          break;

        case "filter":
          const filter = this.audioContext.createBiquadFilter();
          filter.type = "lowpass";
          filter.frequency.value = 5000;
          effectNode = filter;
          break;

        default:
          return null;
      }

      const effect: AudioEffect = {
        id: `${routeId}-${effectType}-${Date.now()}`,
        type: effectType,
        node: effectNode,
        enabled: true,
        parameters: {},
      };

      // Insert effect into the route's audio chain
      route.gainNode.disconnect();
      route.gainNode.connect(effectNode);

      const targetNode = this.outputNodes.get(route.targetNodeId);
      if (targetNode) {
        effectNode.connect(targetNode.gainNode!);
      }

      route.effects.push(effect);
      console.log("[v0] Added effect to route:", effect.id);

      return effect;
    } catch (error) {
      console.error("[v0] Failed to add effect:", error);
      return null;
    }
  }

  async start(): Promise<boolean> {
    if (!this.audioContext) {
      await this.initializeAudioContext();
    }

    if (this.audioContext?.state === "suspended") {
      try {
        await this.audioContext.resume();
        console.log("[v0] Audio context resumed");
        return true;
      } catch (error) {
        console.error("[v0] Failed to resume audio context:", error);
        return false;
      }
    }

    return this.isInitialized;
  }

  async stop(): Promise<void> {
    this.stopLevelMonitoring();

    // Stop all media streams
    for (const [, node] of this.inputNodes) {
      if (node.mediaStream) {
        node.mediaStream.getTracks().forEach((track) => track.stop());
      }
    }

    // Clear all nodes and routes
    this.inputNodes.clear();
    this.outputNodes.clear();
    this.routes.clear();

    if (this.audioContext && this.audioContext.state !== "closed") {
      await this.audioContext.close();
      this.audioContext = null;
    }

    this.isInitialized = false;
    console.log("[v0] Audio engine stopped");
  }

  getStatus() {
    return {
      isInitialized: this.isInitialized,
      contextState: this.audioContext?.state,
      inputNodes: this.inputNodes.size,
      outputNodes: this.outputNodes.size,
      activeRoutes: this.routes.size,
      sampleRate: this.audioContext?.sampleRate,
    };
  }
}

// Singleton instance
export const audioEngine = new AudioRoutingEngine();
