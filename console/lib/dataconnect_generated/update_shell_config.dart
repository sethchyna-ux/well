part of 'generated.dart';

class UpdateShellConfigVariablesBuilder {
  String id;
  Optional<String> _prompt = Optional.optional(nativeFromJson, nativeToJson);

  final FirebaseDataConnect _dataConnect;  UpdateShellConfigVariablesBuilder prompt(String? t) {
   _prompt.value = t;
   return this;
  }

  UpdateShellConfigVariablesBuilder(this._dataConnect, {required  this.id,});
  Deserializer<UpdateShellConfigData> dataDeserializer = (dynamic json)  => UpdateShellConfigData.fromJson(jsonDecode(json));
  Serializer<UpdateShellConfigVariables> varsSerializer = (UpdateShellConfigVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<UpdateShellConfigData, UpdateShellConfigVariables>> execute() {
    return ref().execute();
  }

  MutationRef<UpdateShellConfigData, UpdateShellConfigVariables> ref() {
    UpdateShellConfigVariables vars= UpdateShellConfigVariables(id: id,prompt: _prompt,);
    return _dataConnect.mutation("UpdateShellConfig", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class UpdateShellConfigShellConfigUpdate {
  final String id;
  UpdateShellConfigShellConfigUpdate.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateShellConfigShellConfigUpdate otherTyped = other as UpdateShellConfigShellConfigUpdate;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  UpdateShellConfigShellConfigUpdate({
    required this.id,
  });
}

@immutable
class UpdateShellConfigData {
  final UpdateShellConfigShellConfigUpdate? shellConfig_update;
  UpdateShellConfigData.fromJson(dynamic json):
  
  shellConfig_update = json['shellConfig_update'] == null ? null : UpdateShellConfigShellConfigUpdate.fromJson(json['shellConfig_update']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateShellConfigData otherTyped = other as UpdateShellConfigData;
    return shellConfig_update == otherTyped.shellConfig_update;
    
  }
  @override
  int get hashCode => shellConfig_update.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    if (shellConfig_update != null) {
      json['shellConfig_update'] = shellConfig_update!.toJson();
    }
    return json;
  }

  UpdateShellConfigData({
    this.shellConfig_update,
  });
}

@immutable
class UpdateShellConfigVariables {
  final String id;
  late final Optional<String>prompt;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  UpdateShellConfigVariables.fromJson(Map<String, dynamic> json):
  
  id = nativeFromJson<String>(json['id']) {
  
  
  
    prompt = Optional.optional(nativeFromJson, nativeToJson);
    prompt.value = json['prompt'] == null ? null : nativeFromJson<String>(json['prompt']);
  
  }
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final UpdateShellConfigVariables otherTyped = other as UpdateShellConfigVariables;
    return id == otherTyped.id && 
    prompt == otherTyped.prompt;
    
  }
  @override
  int get hashCode => Object.hashAll([id.hashCode, prompt.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    if(prompt.state == OptionalState.set) {
      json['prompt'] = prompt.toJson();
    }
    return json;
  }

  UpdateShellConfigVariables({
    required this.id,
    required this.prompt,
  });
}

