# Pattern
![Pattern](./images/pattern.svg)

# Types
![Types](./images/types.svg)

# Sensor <> Controller
- the sensor __publishes__ commands to register/unregister itself to the controller

```protobuf
message EntityDiscoveryCommand {
	enum Command {
		REGISTER           = 0;
		UNREGISTER         = 1;
	}
	enum EntityType {
		SENSOR             = 0;
		ACTUATOR           = 1;
	}
	Command command        = 1;
	EntityType entity_type = 2;
	string entity_name     = 3;
}
```

- the sensor __publishes__ sensor data in the specified update frequency to the controller

```protobuf
message SensorMeasurement {
	oneof value {
		TemperatureSensorMeasurement temperature = 1;
		HumiditySensorMeasurement humidity       = 2;
	}
	string unit = 3;
}

message TemperatureSensorMeasurement {
	float temperature = 1;
}

message HumiditySensorMeasurement {
	float humidity = 1;
}
```

- the sensor can be __requested__ to change the update frequency

```protobuf
message SensorConfiguration {
	float update_frequency_hz = 1;
}

message ResponseCode {
	enum Code {
		OK    = 0;
		ERROR = 1;
	}
	Code code = 1;
}
```

# Actuator <> Controller
- the actuator __publishes__ commands to register/unregister itself to the controller
- the controller can __request__ the actuator to change its state
- the actuator __publishes__ its state to the controller

```protobuf
message ActuatorState {
	oneof state {
		LightActuatorState light                      = 1;
		AirConditioningActuatorState air_conditioning = 2;
	}
}

message LightActuatorState {
	float brightness = 1;
}

message AirConditioningActuatorState {
	bool on = 1;
}
```

# Controller <> Client
- the client can __request__ the current state of the system, including active sensors/actuators, sensor values, and actuator states from the client

```protobuf
message SystemStateQuery {
}

message SystemState {
	map<string, SensorMeasurement> sensors = 1;
	map<string, ActuatorState> actuators   = 2;
}
```

- the client can __request__ the system to set an actuator target value or the sensor update frequency (the request is forwarded to the actuator/sensor)

```protobuf
message NamedEntityState {
	string entity_name = 1;
	oneof state {
		SensorConfiguration sensor_configuration = 2;
		ActuatorState actuator_state             = 3;
	}
}
```