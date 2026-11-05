part of 'generated.dart';

class UpdateDeviceSyncVariablesBuilder {
  String id;
  Optional<String> _profileId = Optional.optional(nativeFromJson, nativeToJson);

  final FirebaseDataConnect _dataConnect;  UpdateDeviceSyncVariablesBuilder profileId(String? t) {
   _profileId.value = t;
   return this;
  }

  UpdateDeviceSyncVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<UpdateDeviceSyncData> dataDeserializer = (dynamic json)  => UpdateDeviceSyncData.fromJson(jsonDecode(json));
  Serializer<UpdateDeviceSyncVariables> varsSerializer = (UpdateDeviceSyncVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<UpdateDeviceSyncData, UpdateDeviceSyncVariables>> execute() {
    return ref().execute();
  }

  MutationRef<UpdateDeviceSyncData, UpdateDeviceSyncVariables> ref() {
    UpdateDeviceSyncVariables vars= UpdateDeviceSyncVariables(id: id,profileId: _profileId,);
    return _dataConnect.mutation("UpdateDeviceSync", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class UpdateDeviceSyncDeviceSyncUpdate {
  final String id;
  UpdateDeviceSyncDeviceSyncUpdate.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateDeviceSyncDeviceSyncUpdate otherTyped = other as UpdateDeviceSyncDeviceSyncUpdate;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  UpdateDeviceSyncDeviceSyncUpdate({
    required this.id,
  });
}

@immutable
class UpdateDeviceSyncData {
  final UpdateDeviceSyncDeviceSyncUpdate? deviceSync_update;
  UpdateDeviceSyncData.fromJson(dynamic json):
  
  deviceSync_update = json['deviceSync_update'] == null ? null : UpdateDeviceSyncDeviceSyncUpdate.fromJson(json['deviceSync_update']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateDeviceSyncData otherTyped = other as UpdateDeviceSyncData;
    return deviceSync_update == otherTyped.deviceSync_update;
    
  }
  @override
  int get hashCode => deviceSync_update.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (deviceSync_update != null) {
      json['deviceSync_update'] = deviceSync_update!.toJson();
    }
    return json;
  }

  UpdateDeviceSyncData({
    this.deviceSync_update,
  });
}

@immutable
class UpdateDeviceSyncVariables {
  final String id;
  late final Optional<String>profileId;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  UpdateDeviceSyncVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']) {
  
  
  
    profileId = Optional.optional(nativeFromJson, nativeToJson);
    profileId.value = json['profileId'] == null ? null : nativeFromJson<String>(json['profileId']);
  
  }
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateDeviceSyncVariables otherTyped = other as UpdateDeviceSyncVariables;
    return id == otherTyped.id && 
    profileId == otherTyped.profileId;
    
  }
  @override
  int get hashCode => Object.hashAll([id.hashCode, profileId.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    if(profileId.state == OptionalState.set) {
      json['profileId'] = profileId.toJson();
    }
    return json;
  }

  UpdateDeviceSyncVariables({
    required this.id,
    required this.profileId,
  });
}

