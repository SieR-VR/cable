import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { AudioConnection } from "./audio-multiplexer";
import type { AudioRoutingEngine, AudioEffect } from "@/lib/audio-engine";
import {
  Volume2,
  EqualIcon as Equalizer,
  Zap,
  Settings,
  Plus,
  Trash2,
  BarChart3,
} from "lucide-react";

interface AudioProcessingControlsProps {
  connections: AudioConnection[];
  onConnectionChange: (connections: AudioConnection[]) => void;
  audioEngine: AudioRoutingEngine;
}

interface EffectParameters {
  [key: string]: number;
}

export function AudioProcessingControls({
  connections,
  onConnectionChange,
  audioEngine,
}: AudioProcessingControlsProps) {
  const [selectedConnection, setSelectedConnection] = useState<string | null>(
    null
  );
  const [connectionEffects, setConnectionEffects] = useState<
    Map<string, AudioEffect[]>
  >(new Map());
  const [effectParameters, setEffectParameters] = useState<
    Map<string, EffectParameters>
  >(new Map());
  const [processingMode, setProcessingMode] = useState<
    "realtime" | "buffered" | "offline"
  >("realtime");
  const [globalSettings, setGlobalSettings] = useState({
    masterLimiterThreshold: -3,
    outputGain: 0,
    highQualityProcessing: true,
    latencyCompensation: true,
  });

  const updateConnectionVolume = (connectionId: string, volume: number) => {
    onConnectionChange(
      connections.map((c) => (c.id === connectionId ? { ...c, volume } : c))
    );
    audioEngine.setRouteVolume(connectionId, volume);
  };

  const toggleConnectionActive = (connectionId: string) => {
    const connection = connections.find((c) => c.id === connectionId);
    if (!connection) return;

    const newActiveState = !connection.isActive;
    onConnectionChange(
      connections.map((c) =>
        c.id === connectionId ? { ...c, isActive: newActiveState } : c
      )
    );

    if (!newActiveState) {
      audioEngine.removeRoute(connectionId);
    }
  };

  const addEffect = async (
    connectionId: string,
    effectType: AudioEffect["type"]
  ) => {
    const effect = audioEngine.addEffect(connectionId, effectType);
    if (effect) {
      const currentEffects = connectionEffects.get(connectionId) || [];
      setConnectionEffects(
        new Map(
          connectionEffects.set(connectionId, [...currentEffects, effect])
        )
      );

      // Initialize default parameters
      const defaultParams = getDefaultParameters(effectType);
      setEffectParameters(
        new Map(effectParameters.set(effect.id, defaultParams))
      );
    }
  };

  const removeEffect = (connectionId: string, effectId: string) => {
    const currentEffects = connectionEffects.get(connectionId) || [];
    const updatedEffects = currentEffects.filter((e) => e.id !== effectId);
    setConnectionEffects(
      new Map(connectionEffects.set(connectionId, updatedEffects))
    );
  };

  const getDefaultParameters = (
    effectType: AudioEffect["type"]
  ): EffectParameters => {
    switch (effectType) {
      case "equalizer":
        return { low: 0, mid: 0, high: 0 };
      case "compressor":
        return {
          threshold: -24,
          ratio: 4,
          attack: 0.003,
          release: 0.25,
          knee: 30,
        };
      case "reverb":
        return { roomSize: 50, damping: 50, wetLevel: 30, dryLevel: 70 };
      case "delay":
        return { delayTime: 300, feedback: 30, wetLevel: 25 };
      case "filter":
        return { frequency: 5000, resonance: 1, type: 0 }; // 0=lowpass, 1=highpass, 2=bandpass
      default:
        return {};
    }
  };

  const updateEffectParameter = (
    effectId: string,
    paramName: string,
    value: number
  ) => {
    const currentParams = effectParameters.get(effectId) || {};
    const updatedParams = { ...currentParams, [paramName]: value };
    setEffectParameters(new Map(effectParameters.set(effectId, updatedParams)));

    // Apply parameter change to audio engine
    // This would require extending the audio engine to support parameter updates
    console.log("[v0] Updated effect parameter:", effectId, paramName, value);
  };

  const ConnectionProcessor = ({
    connection,
  }: {
    connection: AudioConnection;
  }) => {
    const effects = connectionEffects.get(connection.id) || [];

    return (
      <Card className="bg-card border-border">
        <CardHeader className="pb-3">
          <div className="flex items-center justify-between">
            <CardTitle className="text-sm font-medium">
              Route: {connection.sourceId.split("-").pop()} →{" "}
              {connection.targetId.split("-").pop()}
            </CardTitle>
            <div className="flex items-center gap-2">
              <Badge
                variant={connection.isActive ? "default" : "secondary"}
                className="text-xs"
              >
                {connection.isActive ? "Active" : "Inactive"}
              </Badge>
              <Switch
                checked={connection.isActive}
                onCheckedChange={() => toggleConnectionActive(connection.id)}
              />
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Volume Control */}
          <div className="space-y-2">
            <div className="flex items-center justify-between text-sm">
              <span className="flex items-center gap-1">
                <Volume2 className="h-3 w-3" />
                Volume
              </span>
              <span className="text-muted-foreground">
                {connection.volume}%
              </span>
            </div>
            <Slider
              value={[connection.volume]}
              onValueChange={(value) =>
                updateConnectionVolume(connection.id, value[0])
              }
              max={100}
              step={1}
              disabled={!connection.isActive}
              className="w-full"
            />
          </div>

          {/* Effects Chain */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-sm font-medium">
                <Zap className="h-3 w-3" />
                Effects Chain ({effects.length})
              </div>
              <Select
                onValueChange={(value) =>
                  addEffect(connection.id, value as AudioEffect["type"])
                }
              >
                <SelectTrigger className="w-32 h-8">
                  <Plus className="h-3 w-3" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="equalizer">EQ</SelectItem>
                  <SelectItem value="compressor">Compressor</SelectItem>
                  <SelectItem value="reverb">Reverb</SelectItem>
                  <SelectItem value="delay">Delay</SelectItem>
                  <SelectItem value="filter">Filter</SelectItem>
                </SelectContent>
              </Select>
            </div>

            {effects.map((effect, index) => (
              <Card key={effect.id} className="bg-muted/50">
                <CardContent className="p-3">
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium">
                        {effect.type.toUpperCase()}
                      </span>
                      <Badge variant="outline" className="text-xs">
                        #{index + 1}
                      </Badge>
                    </div>
                    <div className="flex items-center gap-1">
                      <Switch
                        checked={effect.enabled}
                        disabled={!connection.isActive}
                      />
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => removeEffect(connection.id, effect.id)}
                        className="h-6 w-6 p-0"
                      >
                        <Trash2 className="h-3 w-3" />
                      </Button>
                    </div>
                  </div>

                  {effect.enabled && connection.isActive && (
                    <EffectControls
                      effect={effect}
                      parameters={effectParameters.get(effect.id) || {}}
                      onParameterChange={(paramName, value) =>
                        updateEffectParameter(effect.id, paramName, value)
                      }
                    />
                  )}
                </CardContent>
              </Card>
            ))}
          </div>
        </CardContent>
      </Card>
    );
  };

  const EffectControls = ({
    effect,
    parameters,
    onParameterChange,
  }: {
    effect: AudioEffect;
    parameters: EffectParameters;
    onParameterChange: (paramName: string, value: number) => void;
  }) => {
    switch (effect.type) {
      case "equalizer":
        return (
          <div className="grid grid-cols-3 gap-2">
            {["low", "mid", "high"].map((band) => (
              <div key={band} className="text-center">
                <div className="text-xs text-muted-foreground mb-1 capitalize">
                  {band}
                </div>
                <div className="h-20 flex items-end justify-center">
                  <Slider
                    value={[parameters[band] || 0]}
                    onValueChange={(value) => onParameterChange(band, value[0])}
                    max={12}
                    min={-12}
                    step={0.1}
                    orientation="vertical"
                    className="h-16"
                  />
                </div>
                <div className="text-xs text-muted-foreground mt-1">
                  {(parameters[band] || 0).toFixed(1)}dB
                </div>
              </div>
            ))}
          </div>
        );

      case "compressor":
        return (
          <div className="grid grid-cols-2 gap-3">
            <div>
              <div className="text-xs text-muted-foreground mb-1">
                Threshold
              </div>
              <Slider
                value={[parameters.threshold || -24]}
                onValueChange={(value) =>
                  onParameterChange("threshold", value[0])
                }
                max={0}
                min={-60}
                step={1}
              />
              <div className="text-xs text-center mt-1">
                {parameters.threshold || -24}dB
              </div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground mb-1">Ratio</div>
              <Slider
                value={[parameters.ratio || 4]}
                onValueChange={(value) => onParameterChange("ratio", value[0])}
                max={20}
                min={1}
                step={0.1}
              />
              <div className="text-xs text-center mt-1">
                {(parameters.ratio || 4).toFixed(1)}:1
              </div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground mb-1">Attack</div>
              <Slider
                value={[parameters.attack || 0.003]}
                onValueChange={(value) => onParameterChange("attack", value[0])}
                max={0.1}
                min={0.001}
                step={0.001}
              />
              <div className="text-xs text-center mt-1">
                {((parameters.attack || 0.003) * 1000).toFixed(1)}ms
              </div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground mb-1">Release</div>
              <Slider
                value={[parameters.release || 0.25]}
                onValueChange={(value) =>
                  onParameterChange("release", value[0])
                }
                max={2}
                min={0.01}
                step={0.01}
              />
              <div className="text-xs text-center mt-1">
                {((parameters.release || 0.25) * 1000).toFixed(0)}ms
              </div>
            </div>
          </div>
        );

      case "reverb":
        return (
          <div className="space-y-3">
            <div>
              <div className="text-xs text-muted-foreground mb-1">
                Room Size
              </div>
              <Slider
                value={[parameters.roomSize || 50]}
                onValueChange={(value) =>
                  onParameterChange("roomSize", value[0])
                }
                max={100}
                min={0}
                step={1}
              />
              <div className="text-xs text-center mt-1">
                {parameters.roomSize || 50}%
              </div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground mb-1">
                Wet Level
              </div>
              <Slider
                value={[parameters.wetLevel || 30]}
                onValueChange={(value) =>
                  onParameterChange("wetLevel", value[0])
                }
                max={100}
                min={0}
                step={1}
              />
              <div className="text-xs text-center mt-1">
                {parameters.wetLevel || 30}%
              </div>
            </div>
          </div>
        );

      case "delay":
        return (
          <div className="space-y-3">
            <div>
              <div className="text-xs text-muted-foreground mb-1">
                Delay Time
              </div>
              <Slider
                value={[parameters.delayTime || 300]}
                onValueChange={(value) =>
                  onParameterChange("delayTime", value[0])
                }
                max={1000}
                min={10}
                step={10}
              />
              <div className="text-xs text-center mt-1">
                {parameters.delayTime || 300}ms
              </div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground mb-1">Feedback</div>
              <Slider
                value={[parameters.feedback || 30]}
                onValueChange={(value) =>
                  onParameterChange("feedback", value[0])
                }
                max={95}
                min={0}
                step={1}
              />
              <div className="text-xs text-center mt-1">
                {parameters.feedback || 30}%
              </div>
            </div>
          </div>
        );

      case "filter":
        return (
          <div className="space-y-3">
            <div>
              <div className="text-xs text-muted-foreground mb-1">
                Cutoff Frequency
              </div>
              <Slider
                value={[parameters.frequency || 5000]}
                onValueChange={(value) =>
                  onParameterChange("frequency", value[0])
                }
                max={20000}
                min={20}
                step={10}
              />
              <div className="text-xs text-center mt-1">
                {parameters.frequency || 5000}Hz
              </div>
            </div>
            <div>
              <div className="text-xs text-muted-foreground mb-1">
                Resonance
              </div>
              <Slider
                value={[parameters.resonance || 1]}
                onValueChange={(value) =>
                  onParameterChange("resonance", value[0])
                }
                max={30}
                min={0.1}
                step={0.1}
              />
              <div className="text-xs text-center mt-1">
                {(parameters.resonance || 1).toFixed(1)}
              </div>
            </div>
          </div>
        );

      default:
        return (
          <div className="text-xs text-muted-foreground">
            No controls available
          </div>
        );
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold text-foreground">
          Audio Processing
        </h2>
        <div className="flex items-center gap-2">
          <Select
            value={processingMode}
            onValueChange={(value: any) => setProcessingMode(value)}
          >
            <SelectTrigger className="w-32">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="realtime">Real-time</SelectItem>
              <SelectItem value="buffered">Buffered</SelectItem>
              <SelectItem value="offline">Offline</SelectItem>
            </SelectContent>
          </Select>
          <Button variant="outline" size="sm">
            <BarChart3 className="h-4 w-4 mr-2" />
            Analyzer
          </Button>
        </div>
      </div>

      <Tabs defaultValue="connections" className="w-full">
        <TabsList>
          <TabsTrigger value="connections">Connections</TabsTrigger>
          <TabsTrigger value="global">Global Processing</TabsTrigger>
          <TabsTrigger value="presets">Presets</TabsTrigger>
        </TabsList>

        <TabsContent value="connections" className="space-y-4">
          {connections.length === 0 ? (
            <Card className="bg-card/50">
              <CardContent className="p-8 text-center">
                <Equalizer className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
                <h3 className="text-lg font-medium mb-2">
                  No Active Connections
                </h3>
                <p className="text-sm text-muted-foreground">
                  Create audio connections in the Flow View to access processing
                  controls
                </p>
              </CardContent>
            </Card>
          ) : (
            <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
              {connections.map((connection) => (
                <ConnectionProcessor
                  key={connection.id}
                  connection={connection}
                />
              ))}
            </div>
          )}
        </TabsContent>

        <TabsContent value="global" className="space-y-4">
          {/* Global Processing Controls */}
          <Card className="bg-card border-border">
            <CardHeader>
              <CardTitle className="text-lg">Master Processing Chain</CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="grid grid-cols-2 gap-6">
                <div>
                  <div className="text-sm font-medium mb-3">Master Limiter</div>
                  <div className="space-y-3">
                    <div>
                      <div className="flex items-center justify-between text-xs mb-1">
                        <span>Threshold</span>
                        <span>{globalSettings.masterLimiterThreshold} dB</span>
                      </div>
                      <Slider
                        value={[globalSettings.masterLimiterThreshold]}
                        onValueChange={(value) =>
                          setGlobalSettings({
                            ...globalSettings,
                            masterLimiterThreshold: value[0],
                          })
                        }
                        max={0}
                        min={-20}
                        step={0.1}
                      />
                    </div>
                  </div>
                </div>
                <div>
                  <div className="text-sm font-medium mb-3">Output Gain</div>
                  <div className="space-y-3">
                    <div>
                      <div className="flex items-center justify-between text-xs mb-1">
                        <span>Gain</span>
                        <span>{globalSettings.outputGain} dB</span>
                      </div>
                      <Slider
                        value={[globalSettings.outputGain]}
                        onValueChange={(value) =>
                          setGlobalSettings({
                            ...globalSettings,
                            outputGain: value[0],
                          })
                        }
                        max={12}
                        min={-12}
                        step={0.1}
                      />
                    </div>
                  </div>
                </div>
              </div>

              <div className="space-y-4 pt-4 border-t border-border">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-sm font-medium">
                      High Quality Processing
                    </div>
                    <div className="text-xs text-muted-foreground">
                      Use 64-bit floating point precision
                    </div>
                  </div>
                  <Switch
                    checked={globalSettings.highQualityProcessing}
                    onCheckedChange={(checked) =>
                      setGlobalSettings({
                        ...globalSettings,
                        highQualityProcessing: checked,
                      })
                    }
                  />
                </div>
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-sm font-medium">
                      Latency Compensation
                    </div>
                    <div className="text-xs text-muted-foreground">
                      Automatically compensate for processing delays
                    </div>
                  </div>
                  <Switch
                    checked={globalSettings.latencyCompensation}
                    onCheckedChange={(checked) =>
                      setGlobalSettings({
                        ...globalSettings,
                        latencyCompensation: checked,
                      })
                    }
                  />
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="presets" className="space-y-4">
          <Card className="bg-card border-border">
            <CardHeader>
              <CardTitle className="text-lg">Processing Presets</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="text-center py-8">
                <Settings className="h-12 w-12 text-muted-foreground mx-auto mb-4" />
                <h3 className="text-lg font-medium mb-2">
                  Presets Coming Soon
                </h3>
                <p className="text-sm text-muted-foreground">
                  Save and load your favorite processing configurations
                </p>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}
