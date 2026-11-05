part of 'generated.dart';

class CreateShellConfigVariablesBuilder {
  String profileId;
  String prompt;
  String alias;

  final FirebaseDataConnect _dataConnect;
  CreateShellConfigVariablesBuilder(this._dataConnect, {required  this.profileId,required  this.prompt,required  this.alias,});
  Deserializer<CreateShellConfigData> dataDeserializer = (dynamic json)  => CreateShellConfigData.fromJson(jsonDecode(json));
  Serializer<CreateShellConfigVariables> varsSerializer = (CreateShellConfigVariables vars) => jsonEncode(vars.toJson());
  Future<OperationResult<CreateShellConfigData, CreateShellConfigVariables>> execute() {
    return ref().execute();
  }

  MutationRef<CreateShellConfigData, CreateShellConfigVariables> ref() {
    CreateShellConfigVariables vars= CreateShellConfigVariables(profileId: profileId,prompt: prompt,alias: alias,);
    return _dataConnect.mutation("CreateShellConfig", dataDeserializer, varsSerializer, vars);
  }
}

@immutable
class CreateShellConfigShellConfigInsert {
  final String id;
  CreateShellConfigShellConfigInsert.fromJson(dynamic json):
  
  id = nativeFromJson<String>(json['id']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateShellConfigShellConfigInsert otherTyped = other as CreateShellConfigShellConfigInsert;
    return id == otherTyped.id;
    
  }
  @override
  int get hashCode => id.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['id'] = nativeToJson<String>(id);
    return json;
  }

  CreateShellConfigShellConfigInsert({
    required this.id,
  });
}

@immutable
class CreateShellConfigData {
  final CreateShellConfigShellConfigInsert shellConfig_insert;
  CreateShellConfigData.fromJson(dynamic json):
  
  shellConfig_insert = CreateShellConfigShellConfigInsert.fromJson(json['shellConfig_insert']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateShellConfigData otherTyped = other as CreateShellConfigData;
    return shellConfig_insert == otherTyped.shellConfig_insert;
    
  }
  @override
  int get hashCode => shellConfig_insert.hashCode;
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['shellConfig_insert'] = shellConfig_insert.toJson();
    return json;
  }

  CreateShellConfigData({
    required this.shellConfig_insert,
  });
}

@immutable
class CreateShellConfigVariables {
  final String profileId;
  final String prompt;
  final String alias;
  @Deprecated('fromJson is deprecated for Variable classes as they are no longer required for deserialization.')
  CreateShellConfigVariables.fromJson(Map<String, dynamic> json):
  
  profileId = nativeFromJson<String>(json['profileId']),
  prompt = nativeFromJson<String>(json['prompt']),
  alias = nativeFromJson<String>(json['alias']);
  @override
  bool operator ==(Object other) {
    if(identical(this, other)) {
      return true;
    }
    if(other.runtimeType != runtimeType) {
      return false;
    }

    final CreateShellConfigVariables otherTyped = other as CreateShellConfigVariables;
    return profileId == otherTyped.profileId && 
    prompt == otherTyped.prompt && 
    alias == otherTyped.alias;
    
  }
  @override
  int get hashCode => Object.hashAll([profileId.hashCode, prompt.hashCode, alias.hashCode]);
  

  Map<String, dynamic> toJson() {
    Map<String, dynamic> json = {};
    json['profileId'] = nativeToJson<String>(profileId);
    json['prompt'] = nativeToJson<String>(prompt);
    json['alias'] = nativeToJson<String>(alias);
    return json;
  }

  CreateShellConfigVariables({
    required this.profileId,
    required this.prompt,
    required this.alias,
  });
}

