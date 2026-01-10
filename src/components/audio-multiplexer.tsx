"use client";

import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

import { AudioDeviceManager } from "./audio-device-manager";
import { AudioFlowCanvas } from "./audio-flow-canvas";
import { AudioProcessingControls } from "./audio-processing-controls";
import { SettingsDialog } from "./settings-dialog";

import {
  Volume2,
  Settings,
  Play,
  Square,
  Mic,
  AlertCircle,
} from "lucide-react";
import { audioEngine } from "@/lib/audio-engine";
import type { AudioDeviceInfo } from "@/lib/audio-engine";

export interface AudioDevice {
  id: string;
  name: string;
  type: "input" | "output";
  isDefault: boolean;
  isActive: boolean;
  volume: number;
  channels: number;
}

export interface AudioConnection {
  id: string;
  sourceId: string;
  targetId: string;
  volume: number;
  isActive: boolean;
}

export function AudioMultiplexer() {
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [connections, setConnections] = useState<AudioConnection[]>([]);
  const [isProcessing, setIsProcessing] = useState(false);
  const [masterVolume, setMasterVolume] = useState([75]);
  const [engineStatus, setEngineStatus] = useState<any>(null);
  const [audioLevels, setAudioLevels] = useState<Map<string, Float32Array>>(
    new Map()
  );
  const [isAudioInitialized, setIsAudioInitialized] = useState(false);
  const [initializationError, setInitializationError] = useState<string | null>(
    null
  );
  const [isInitializing, setIsInitializing] = useState(false);

  const initializeAudio = async () => {
    setIsInitializing(true);
    setInitializationError(null);

    try {
      // Request microphone permission with user gesture
      await navigator.mediaDevices.getUserMedia({ audio: true });

      const availableDevices = await audioEngine.getAvailableDevices();
      console.log("[v0] Raw available devices:", availableDevices);

      const deviceList: AudioDevice[] = availableDevices.map(
        (device: AudioDeviceInfo) => ({
          id: device.id,
          name: device.name,
          type: device.type,
          isDefault: device.isDefault,
          isActive: false,
          volume: 75,
          channels: device.channels,
        })
      );

      const inputDevices = deviceList.filter((d) => d.type === "input");
      const outputDevices = deviceList.filter((d) => d.type === "output");
      console.log(
        "[v0] Input devices found:",
        inputDevices.length,
        inputDevices
      );
      console.log(
        "[v0] Output devices found:",
        outputDevices.length,
        outputDevices
      );

      setDevices(deviceList);
      setEngineStatus(audioEngine.getStatus());
      setIsAudioInitialized(true);
      console.log(
        "[v0] Audio multiplexer initialized with",
        deviceList.length,
        "devices"
      );
    } catch (error: any) {
      console.error("[v0] Failed to initialize audio:", error);
      let errorMessage = "Failed to initialize audio system";

      if (error.name === "NotAllowedError") {
        errorMessage =
          "Microphone permission denied. Please allow access and try again.";
      } else if (error.name === "NotFoundError") {
        errorMessage = "No audio devices found. Please check your audio setup.";
      } else if (error.name === "NotSupportedError") {
        errorMessage = "Audio features not supported in this browser.";
      }

      setInitializationError(errorMessage);
    } finally {
      setIsInitializing(false);
    }
  };

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (isAudioInitialized) {
        audioEngine.stop();
      }
    };
  }, [isAudioInitialized]);

  if (!isAudioInitialized) {
    return (
      <div className="flex h-screen bg-background text-foreground">
        <div className="flex-1 flex items-center justify-center">
          <div className="max-w-md text-center space-y-6">
            <div className="space-y-2">
              <Mic className="h-16 w-16 mx-auto text-muted-foreground" />
              <h1 className="text-3xl font-bold">Audio Multiplexer</h1>
              <p className="text-muted-foreground">
                Professional audio routing and processing for Windows
              </p>
            </div>

            {initializationError && (
              <div className="flex items-center gap-2 p-4 bg-destructive/10 border border-destructive/20 rounded-lg text-destructive">
                <AlertCircle className="h-5 w-5 flex-shrink-0" />
                <p className="text-sm">{initializationError}</p>
              </div>
            )}

            <div className="space-y-4">
              <Button
                onClick={initializeAudio}
                disabled={isInitializing}
                size="lg"
                className="w-full"
              >
                {isInitializing ? (
                  <>
                    <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-current mr-2" />
                    Initializing Audio...
                  </>
                ) : (
                  <>
                    <Mic className="h-4 w-4 mr-2" />
                    Initialize Audio System
                  </>
                )}
              </Button>

              <p className="text-xs text-muted-foreground">
                This will request access to your microphone to detect audio
                devices. No audio will be recorded without your explicit
                permission.
              </p>
            </div>
          </div>
        </div>
      </div>
    );
  }

  const toggleProcessing = async () => {
    if (!isProcessing) {
      const success = await audioEngine.start();
      if (success) {
        setIsProcessing(true);

        // Start level monitoring
        audioEngine.startLevelMonitoring((levels) => {
          setAudioLevels(levels);
        });

        console.log("[v0] Audio processing started");
      } else {
        console.error("[v0] Failed to start audio processing");
      }
    } else {
      await audioEngine.stop();
      setIsProcessing(false);
      audioEngine.stopLevelMonitoring();
      setAudioLevels(new Map());
      console.log("[v0] Audio processing stopped");
    }

    setEngineStatus(audioEngine.getStatus());
  };

  const updateDeviceVolume = async (deviceId: string, volume: number) => {
    setDevices(
      devices.map((device) =>
        device.id === deviceId ? { ...device, volume } : device
      )
    );

    if (isProcessing) {
      const nodeId = `${
        devices.find((d) => d.id === deviceId)?.type
      }-${deviceId}`;
      audioEngine.setNodeVolume(nodeId, volume);
    }
  };

  const toggleDeviceActive = async (deviceId: string) => {
    const device = devices.find((d) => d.id === deviceId);
    if (!device) return;

    const newActiveState = !device.isActive;
    setDevices(
      devices.map((d) =>
        d.id === deviceId ? { ...d, isActive: newActiveState } : d
      )
    );

    if (isProcessing) {
      if (newActiveState) {
        // Create audio node for the device
        if (device.type === "input") {
          await audioEngine.createInputNode(deviceId);
        } else {
          await audioEngine.createOutputNode(deviceId);
        }
      } else {
        // Remove connections involving this device
        const deviceConnections = connections.filter(
          (c) => c.sourceId === deviceId || c.targetId === deviceId
        );
        deviceConnections.forEach((conn) => {
          audioEngine.removeRoute(conn.id);
        });
        setConnections(
          connections.filter(
            (c) => c.sourceId !== deviceId && c.targetId !== deviceId
          )
        );
      }
    }
  };

  const handleMasterVolumeChange = (value: number[]) => {
    setMasterVolume(value);
    if (isProcessing) {
      audioEngine.setMasterVolume(value[0]);
    }
  };

  const handleConnectionChange = (newConnections: AudioConnection[]) => {
    const addedConnections = newConnections.filter(
      (nc) => !connections.find((c) => c.id === nc.id)
    );
    const removedConnections = connections.filter(
      (c) => !newConnections.find((nc) => nc.id === c.id)
    );

    // Handle added connections
    addedConnections.forEach(async (conn) => {
      if (isProcessing) {
        const sourceDevice = devices.find((d) => d.id === conn.sourceId);
        const targetDevice = devices.find((d) => d.id === conn.targetId);

        if (sourceDevice && targetDevice) {
          const sourceNodeId = `${sourceDevice.type}-${sourceDevice.id}`;
          const targetNodeId = `${targetDevice.type}-${targetDevice.id}`;

          // Ensure nodes exist
          if (sourceDevice.isActive && targetDevice.isActive) {
            audioEngine.createRoute(sourceNodeId, targetNodeId);
          }
        }
      }
    });

    // Handle removed connections
    removedConnections.forEach((conn) => {
      if (isProcessing) {
        audioEngine.removeRoute(conn.id);
      }
    });

    setConnections(newConnections);
  };

  return (
    <div className="flex flex-col w-full h-screen bg-background text-foreground">
      {/* Header */}
      <div className="flex justify-between w-full h-16 border-b border-border bg-background px-6 py-4">
        <div className="flex items-center gap-4">
          <h1 className="text-2xl font-bold text-foreground">
            Audio Multiplexer
          </h1>
          <Badge variant={isProcessing ? "default" : "secondary"}>
            {isProcessing ? "Processing" : "Stopped"}
          </Badge>
          {engineStatus && (
            <div className="flex items-center gap-2 text-xs text-foreground">
              <span>{engineStatus.inputNodes}I</span>
              <span>{engineStatus.outputNodes}O</span>
              <span>{engineStatus.activeRoutes}R</span>
              {engineStatus.sampleRate && (
                <span>{engineStatus.sampleRate}Hz</span>
              )}
            </div>
          )}
        </div>

        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <Volume2 className="h-4 w-4" />
            <span className="text-sm font-medium">Master</span>
            <div className="w-24">
              <Slider
                value={masterVolume}
                onValueChange={handleMasterVolumeChange}
                max={100}
                step={1}
                className="w-full"
              />
            </div>
            <span className="text-sm text-foreground w-8">
              {masterVolume[0]}%
            </span>
          </div>

          <Button
            onClick={toggleProcessing}
            variant={isProcessing ? "destructive" : "default"}
            size="sm"
          >
            {isProcessing ? (
              <>
                <Square className="h-4 w-4 mr-2" />
                Stop
              </>
            ) : (
              <>
                <Play className="h-4 w-4 mr-2" />
                Start
              </>
            )}
          </Button>

          <SettingsDialog audioEngine={audioEngine}>
            <Button variant="outline" size="sm">
              <Settings className="h-4 w-4 mr-2" />
              Settings
            </Button>
          </SettingsDialog>
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 p-2">
        <Tabs defaultValue="flow" className="flex h-full">
          <TabsList>
            <TabsTrigger value="flow">Flow View</TabsTrigger>
            <TabsTrigger value="devices">Device Manager</TabsTrigger>
            <TabsTrigger value="processing">Audio Processing</TabsTrigger>
          </TabsList>

          <TabsContent value="flow" className="flex-1">
            <AudioFlowCanvas
              devices={devices}
              connections={connections}
              onConnectionChange={handleConnectionChange}
              onDeviceVolumeChange={updateDeviceVolume}
              isProcessing={isProcessing}
              audioLevels={audioLevels}
            />
          </TabsContent>

          <TabsContent value="devices" className="">
            <AudioDeviceManager
              devices={devices}
              onDeviceVolumeChange={updateDeviceVolume}
              onDeviceToggle={toggleDeviceActive}
              audioLevels={audioLevels}
            />
          </TabsContent>

          <TabsContent value="processing" className="">
            <AudioProcessingControls
              connections={connections}
              onConnectionChange={handleConnectionChange}
              audioEngine={audioEngine}
            />
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
}
